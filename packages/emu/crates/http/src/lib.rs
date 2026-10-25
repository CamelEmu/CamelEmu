//! Standard HTTP transfer (RFC 7231) backed by reqwest, exposed to C++ only through the C ABI
//! declared in `packages/camel_emu/include/emu_http.h`.
//!
//! Nothing specific to a caller crosses this boundary: request bodies, multipart encoding and the
//! mapping of a response onto a caller's result type stay in the C++ shim of that caller. The unsafe
//! surface is this module, which owns every handle and every response buffer the C side holds.

use std::error::Error;
use std::ffi::{CStr, CString, c_void};
use std::io::{self, Read};
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::time::Duration;

use reqwest::blocking::{Body, Client};
use reqwest::redirect::Policy;
use reqwest::{Method, Url};

const MAX_REDIRECTS: usize = 10;

mod tls;

pub use tls::{EmuTlsContext, emu_tls_context_free, emu_tls_context_new};

/// Two clients differ only in their redirect policy, which reqwest fixes at construction.
pub struct EmuHttp {
    base: String,
    plain: Client,
    following: Client,
}

#[repr(C)]
pub struct EmuHttpHeader {
    pub name: *const c_char,
    pub value: *const c_char,
}

pub type ProvideBody =
    unsafe extern "C" fn(user: *mut c_void, buf: *mut u8, len: usize, eof: *mut i32) -> i32;

#[repr(C)]
pub struct EmuHttpRequest {
    pub method: *const c_char,
    pub path: *const c_char,
    pub headers: *mut EmuHttpHeader,
    pub header_count: usize,
    pub body: *const u8,
    pub body_len: usize,
    pub provide_body: Option<ProvideBody>,
    pub user: *mut c_void,
    pub follow_redirects: i32,
}

#[repr(C)]
pub struct EmuHttpResponse {
    pub status: i32,
    pub headers: *mut EmuHttpHeader,
    pub header_count: usize,
    pub body: *const u8,
    pub body_len: usize,
    pub reason: *const c_char,
    pub error: *const c_char,
}

/// Runs an FFI body, turning a panic into `fallback` so it never unwinds across the C boundary.
fn guard<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(fallback)
}

fn empty_response() -> EmuHttpResponse {
    EmuHttpResponse {
        status: 0,
        headers: ptr::null_mut(),
        header_count: 0,
        body: ptr::null(),
        body_len: 0,
        reason: ptr::null(),
        error: ptr::null(),
    }
}

fn failure(message: &str) -> EmuHttpResponse {
    let message = CString::new(message.replace('\0', " ")).unwrap_or_default();
    EmuHttpResponse {
        error: message.into_raw(),
        ..empty_response()
    }
}

fn error_chain(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

fn build_client(
    timeout: Option<Duration>,
    redirect: Policy,
    tls: Option<&EmuTlsContext>,
) -> Result<Client, reqwest::Error> {
    // The blocking client defaults to a 30 second limit on the whole request.
    let mut builder = Client::builder().use_rustls_tls().redirect(redirect).timeout(timeout);
    if let Some(timeout) = timeout {
        builder = builder.connect_timeout(timeout);
    }
    if let Some(tls) = tls {
        builder = builder.use_preconfigured_tls(tls.config.clone());
    }
    builder.build()
}

/// # Safety
/// `host` must be NULL or a NUL-terminated string. `tls` must be NULL or a live context from
/// `emu_tls_context_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_http_open(
    host: *const c_char,
    timeout_seconds: u32,
    tls: *mut EmuTlsContext,
) -> *mut EmuHttp {
    guard(ptr::null_mut(), || {
        if host.is_null() {
            return ptr::null_mut();
        }
        let tls = unsafe { tls.as_ref() };
        let Ok(host) = unsafe { CStr::from_ptr(host) }.to_str() else {
            return ptr::null_mut();
        };
        let Ok(url) = Url::parse(host) else {
            return ptr::null_mut();
        };
        if !matches!(url.scheme(), "http" | "https") {
            return ptr::null_mut();
        }
        let timeout = (timeout_seconds > 0).then(|| Duration::from_secs(timeout_seconds.into()));
        let (Ok(plain), Ok(following)) = (
            build_client(timeout, Policy::none(), tls),
            build_client(timeout, Policy::limited(MAX_REDIRECTS), tls),
        ) else {
            return ptr::null_mut();
        };
        Box::into_raw(Box::new(EmuHttp {
            base: host.trim_end_matches('/').to_owned(),
            plain,
            following,
        }))
    })
}

/// # Safety
/// `http` must be NULL or a handle returned by [`emu_http_open`] that has not been closed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_http_close(http: *mut EmuHttp) {
    guard((), || {
        if !http.is_null() {
            drop(unsafe { Box::from_raw(http) });
        }
    });
}

struct CallbackBody {
    provide: ProvideBody,
    user: *mut c_void,
    done: bool,
}

// The callback runs on a reqwest worker thread while `emu_http_request` blocks the caller, so the
// caller's `user` pointer is never touched concurrently.
unsafe impl Send for CallbackBody {}

impl Read for CallbackBody {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.done || buf.is_empty() {
            return Ok(0);
        }
        let mut eof = 0;
        let written = unsafe { (self.provide)(self.user, buf.as_mut_ptr(), buf.len(), &mut eof) };
        let written = usize::try_from(written)
            .map_err(|_| io::Error::other("provide_body reported a failure"))?
            .min(buf.len());
        self.done = eof != 0;
        Ok(written)
    }
}

/// # Safety
/// `text` must be NULL or a NUL-terminated string.
unsafe fn c_str<'a>(text: *const c_char) -> Option<&'a str> {
    if text.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(text) }.to_str().ok()
}

/// # Safety
/// `request` must satisfy the contract of [`emu_http_request`].
unsafe fn perform(http: &EmuHttp, request: &EmuHttpRequest) -> Result<EmuHttpResponse, String> {
    let method = unsafe { c_str(request.method) }.ok_or("missing request method")?;
    let method = Method::from_bytes(method.as_bytes()).map_err(|error| error.to_string())?;
    let path = unsafe { c_str(request.path) }.ok_or("missing request path")?;
    let client = if request.follow_redirects != 0 {
        &http.following
    } else {
        &http.plain
    };

    let mut builder = client.request(method, format!("{}{}", http.base, path));
    if request.header_count > 0 && !request.headers.is_null() {
        let headers = unsafe { std::slice::from_raw_parts(request.headers, request.header_count) };
        for header in headers {
            let name = unsafe { c_str(header.name) }.ok_or("invalid request header name")?;
            let value = unsafe { c_str(header.value) }.ok_or("invalid request header value")?;
            builder = builder.header(name, value);
        }
    }
    if let Some(provide) = request.provide_body {
        let reader = CallbackBody {
            provide,
            user: request.user,
            done: false,
        };
        builder = builder.body(match request.body_len {
            0 => Body::new(reader),
            len => Body::sized(reader, len as u64),
        });
    } else if !request.body.is_null() && request.body_len > 0 {
        let body = unsafe { std::slice::from_raw_parts(request.body, request.body_len) };
        builder = builder.body(body.to_vec());
    }

    let response = builder.send().map_err(|error| error_chain(&error))?;
    let status = response.status();
    let reason = CString::new(status.canonical_reason().unwrap_or("")).unwrap_or_default();
    let headers: Vec<EmuHttpHeader> = response
        .headers()
        .iter()
        .map(|(name, value)| EmuHttpHeader {
            name: CString::new(name.as_str()).unwrap_or_default().into_raw(),
            value: CString::new(value.as_bytes()).unwrap_or_default().into_raw(),
        })
        .collect();
    let body = response.bytes().map_err(|error| error_chain(&error))?;

    let header_count = headers.len();
    let headers = Box::into_raw(headers.into_boxed_slice()) as *mut EmuHttpHeader;
    let body_len = body.len();
    let body = if body_len == 0 {
        ptr::null()
    } else {
        Box::into_raw(body.to_vec().into_boxed_slice()) as *const u8
    };
    Ok(EmuHttpResponse {
        status: i32::from(status.as_u16()),
        headers,
        header_count,
        body,
        body_len,
        reason: reason.into_raw(),
        error: ptr::null(),
    })
}

/// # Safety
/// `http` must be a live handle from [`emu_http_open`]. `request` must point to a valid request:
/// its strings NUL-terminated, `headers` and `body` readable for their counts, and `provide_body`
/// valid to call with `user` until this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_http_request(
    http: *mut EmuHttp,
    request: *const EmuHttpRequest,
) -> EmuHttpResponse {
    guard(failure("internal panic"), || {
        if http.is_null() || request.is_null() {
            return failure("null argument");
        }
        let (http, request) = unsafe { (&*http, &*request) };
        unsafe { perform(http, request) }.unwrap_or_else(|message| failure(&message))
    })
}

/// # Safety
/// `response` must be NULL or point to a value returned by [`emu_http_request`] that has not been
/// freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_http_response_free(response: *mut EmuHttpResponse) {
    guard((), || {
        if response.is_null() {
            return;
        }
        let response = unsafe { &mut *response };
        unsafe {
            if !response.headers.is_null() {
                let headers = Box::from_raw(ptr::slice_from_raw_parts_mut(
                    response.headers,
                    response.header_count,
                ));
                for header in headers.iter() {
                    drop(CString::from_raw(header.name as *mut c_char));
                    drop(CString::from_raw(header.value as *mut c_char));
                }
            }
            if !response.body.is_null() {
                drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                    response.body as *mut u8,
                    response.body_len,
                )));
            }
            if !response.reason.is_null() {
                drop(CString::from_raw(response.reason as *mut c_char));
            }
            if !response.error.is_null() {
                drop(CString::from_raw(response.error as *mut c_char));
            }
        }
        *response = empty_response();
    });
}
