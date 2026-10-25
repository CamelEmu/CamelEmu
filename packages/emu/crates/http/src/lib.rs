//! Standard HTTP transfer (RFC 7231) over hyper, exposed to C++ only through the C ABI declared in
//! `packages/native/include/emu_http.h`.
//!
//! Nothing specific to a caller crosses this boundary: request bodies, multipart encoding and the
//! mapping of a response onto a caller's result type stay in the C++ shim of that caller. The unsafe
//! surface is this module, which owns every handle and every response buffer the C side holds.

mod tls;

use std::error::Error;
use std::ffi::{CStr, CString, c_void};
use std::future::Future;
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use std::{io, ptr};

use bytes::Bytes;
use http::uri::{PathAndQuery, Scheme};
use http::{HeaderName, HeaderValue, Method, StatusCode, Uri, header};
use http_body::{Body, Frame, SizeHint};
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper_openssl::SslStream;
use hyper_openssl::client::legacy::MaybeHttpsStream;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::{TokioExecutor, TokioIo};
use openssl::ssl::{Ssl, SslContext};
use tokio::net::TcpStream;
use tokio::runtime::{Builder, Runtime};
use tower_service::Service;

pub use tls::{EmuTlsContext, emu_tls_context_free, emu_tls_context_new};

const MAX_REDIRECTS: usize = 10;
const CHUNK_SIZE: usize = 16 * 1024;

type BoxedError = Box<dyn Error + Send + Sync>;
type RequestBody = UnsyncBoxBody<Bytes, io::Error>;

pub struct EmuHttp {
    base: String,
    timeout: Option<Duration>,
    runtime: Runtime,
    client: Client<Connector, RequestBody>,
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

/// Opens a TCP connection and, for https, performs the TLS handshake on a context that carries
/// nothing but the settings the caller asked for.
#[derive(Clone)]
struct Connector {
    http: HttpConnector,
    context: Arc<SslContext>,
}

impl Service<Uri> for Connector {
    type Response = MaybeHttpsStream<TokioIo<TcpStream>>;
    type Error = BoxedError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.http.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        let secure = uri.scheme() == Some(&Scheme::HTTPS);
        let connect = self.http.call(uri.clone());
        let context = self.context.clone();
        Box::pin(async move {
            let connection = connect.await?;
            if !secure {
                return Ok(MaybeHttpsStream::Http(connection));
            }
            let host = uri.host().ok_or("the URL has no host")?;
            let mut ssl = Ssl::new(&context)?;
            ssl.set_hostname(host)?;
            ssl.param_mut().set_host(host)?;
            let mut stream = SslStream::new(ssl, connection)?;
            Pin::new(&mut stream).connect().await?;
            Ok(MaybeHttpsStream::Https(stream))
        })
    }
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
        let Ok(host) = unsafe { CStr::from_ptr(host) }.to_str() else {
            return ptr::null_mut();
        };
        let Ok(uri) = host.parse::<Uri>() else {
            return ptr::null_mut();
        };
        if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.authority().is_none() {
            return ptr::null_mut();
        }
        let context = match unsafe { tls.as_ref() } {
            Some(tls) => tls.context.clone(),
            None => match tls::default_context() {
                Ok(context) => context,
                Err(_) => return ptr::null_mut(),
            },
        };
        let Ok(runtime) = Builder::new_current_thread().enable_all().build() else {
            return ptr::null_mut();
        };
        let timeout = (timeout_seconds > 0).then(|| Duration::from_secs(timeout_seconds.into()));
        let mut http = HttpConnector::new();
        http.enforce_http(false);
        http.set_connect_timeout(timeout);
        let connector = Connector {
            http,
            context: Arc::new(context),
        };
        let client = {
            let _entered = runtime.enter();
            Client::builder(TokioExecutor::new())
                .http1_title_case_headers(true)
                .build(connector)
        };
        Box::into_raw(Box::new(EmuHttp {
            base: host.trim_end_matches('/').to_owned(),
            timeout,
            runtime,
            client,
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

/// Hands out the body of a request piece by piece from the caller. `length` is the total size when
/// the caller knows it, and the body is sent chunked otherwise.
struct CallbackBody {
    provide: ProvideBody,
    user: *mut c_void,
    length: Option<u64>,
    done: bool,
}

/// The callback runs on the thread that called `emu_http_request`, which is blocked inside the
/// runtime until the request ends, so the caller's `user` pointer is never touched concurrently.
unsafe impl Send for CallbackBody {}

impl Body for CallbackBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        loop {
            if self.done {
                return Poll::Ready(None);
            }
            let mut buffer = vec![0u8; CHUNK_SIZE];
            let mut eof = 0;
            let written =
                unsafe { (self.provide)(self.user, buffer.as_mut_ptr(), buffer.len(), &mut eof) };
            let Ok(written) = usize::try_from(written) else {
                return Poll::Ready(Some(Err(io::Error::other(
                    "provide_body reported a failure",
                ))));
            };
            self.done = eof != 0;
            buffer.truncate(written.min(CHUNK_SIZE));
            if !buffer.is_empty() {
                return Poll::Ready(Some(Ok(Frame::data(Bytes::from(buffer)))));
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.done
    }

    fn size_hint(&self) -> SizeHint {
        self.length.map_or_else(SizeHint::default, SizeHint::with_exact)
    }
}

enum BodySource {
    Empty,
    Bytes(Bytes),
    Callback(CallbackBody),
}

impl BodySource {
    fn into_body(self) -> RequestBody {
        match self {
            BodySource::Empty => Empty::<Bytes>::new().map_err(|never| match never {}).boxed_unsync(),
            BodySource::Bytes(bytes) => Full::new(bytes).map_err(|never| match never {}).boxed_unsync(),
            BodySource::Callback(body) => body.boxed_unsync(),
        }
    }

    fn replayable(&self) -> Option<BodySource> {
        match self {
            BodySource::Empty => Some(BodySource::Empty),
            BodySource::Bytes(bytes) => Some(BodySource::Bytes(bytes.clone())),
            BodySource::Callback(_) => None,
        }
    }
}

struct Prepared {
    method: Method,
    uri: Uri,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: BodySource,
    follow_redirects: bool,
}

struct Received {
    status: StatusCode,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Bytes,
}

/// Resolves the `Location` of a redirect against the URL it answered.
fn resolve(current: &Uri, location: &str) -> Option<Uri> {
    let target: Uri = location.parse().ok()?;
    if target.scheme().is_some() {
        return Some(target);
    }
    let path = if location.starts_with('/') {
        location.to_owned()
    } else {
        let directory = current.path().rsplit_once('/').map_or("", |(directory, _)| directory);
        format!("{directory}/{location}")
    };
    let mut parts = current.clone().into_parts();
    parts.path_and_query = Some(path.parse::<PathAndQuery>().ok()?);
    Uri::from_parts(parts).ok()
}

async fn execute(client: &Client<Connector, RequestBody>, prepared: Prepared) -> Result<Received, String> {
    let Prepared {
        mut method,
        mut uri,
        mut headers,
        body,
        follow_redirects,
    } = prepared;
    let mut body = Some(body);

    for hop in 0.. {
        let source = body.take().unwrap_or(BodySource::Empty);
        let next = source.replayable();
        let mut request = http::Request::builder()
            .method(method.clone())
            .uri(uri.clone())
            .body(source.into_body())
            .map_err(|error| error.to_string())?;
        for (name, value) in &headers {
            request.headers_mut().append(name.clone(), value.clone());
        }
        let response = client.request(request).await.map_err(|error| error_chain(&error))?;

        let status = response.status();
        let location = response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|location| resolve(&uri, location));
        if let (true, true, Some(location)) = (follow_redirects, hop < MAX_REDIRECTS, location) {
            match status.as_u16() {
                301..=303 => {
                    if method != Method::GET && method != Method::HEAD {
                        method = Method::GET;
                    }
                    headers.retain(|(name, _)| {
                        ![header::CONTENT_TYPE, header::CONTENT_LENGTH, header::TRANSFER_ENCODING]
                            .contains(name)
                    });
                    body = Some(BodySource::Empty);
                    uri = location;
                    continue;
                }
                307 | 308 if next.is_some() => {
                    body = next;
                    uri = location;
                    continue;
                }
                _ => {}
            }
        }

        let (parts, incoming) = response.into_parts();
        let collected = incoming.collect().await.map_err(|error| error_chain(&error))?;
        return Ok(Received {
            status: parts.status,
            headers: parts
                .headers
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
            body: collected.to_bytes(),
        });
    }
    unreachable!()
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
unsafe fn prepare(http: &EmuHttp, request: &EmuHttpRequest) -> Result<Prepared, String> {
    let method = unsafe { c_str(request.method) }.ok_or("missing request method")?;
    let method = Method::from_bytes(method.as_bytes()).map_err(|error| error.to_string())?;
    let path = unsafe { c_str(request.path) }.ok_or("missing request path")?;
    let uri = format!("{}{}", http.base, path)
        .parse::<Uri>()
        .map_err(|error| error.to_string())?;

    let mut headers = Vec::new();
    if request.header_count > 0 && !request.headers.is_null() {
        let raw = unsafe { std::slice::from_raw_parts(request.headers, request.header_count) };
        for pair in raw {
            let name = unsafe { c_str(pair.name) }.ok_or("invalid request header name")?;
            let value = unsafe { c_str(pair.value) }.ok_or("invalid request header value")?;
            headers.push((
                HeaderName::from_bytes(name.as_bytes()).map_err(|error| error.to_string())?,
                HeaderValue::from_str(value).map_err(|error| error.to_string())?,
            ));
        }
    }

    let body = if let Some(provide) = request.provide_body {
        BodySource::Callback(CallbackBody {
            provide,
            user: request.user,
            length: (request.body_len > 0).then_some(request.body_len as u64),
            done: false,
        })
    } else if !request.body.is_null() && request.body_len > 0 {
        let bytes = unsafe { std::slice::from_raw_parts(request.body, request.body_len) };
        BodySource::Bytes(Bytes::copy_from_slice(bytes))
    } else {
        BodySource::Empty
    };

    Ok(Prepared {
        method,
        uri,
        headers,
        body,
        follow_redirects: request.follow_redirects != 0,
    })
}

fn into_response(received: Received) -> EmuHttpResponse {
    let reason = CString::new(received.status.canonical_reason().unwrap_or("")).unwrap_or_default();
    let headers: Vec<EmuHttpHeader> = received
        .headers
        .iter()
        .map(|(name, value)| EmuHttpHeader {
            name: CString::new(name.as_str()).unwrap_or_default().into_raw(),
            value: CString::new(value.as_bytes()).unwrap_or_default().into_raw(),
        })
        .collect();
    let header_count = headers.len();
    let headers = Box::into_raw(headers.into_boxed_slice()) as *mut EmuHttpHeader;
    let body_len = received.body.len();
    let body = if body_len == 0 {
        ptr::null()
    } else {
        Box::into_raw(received.body.to_vec().into_boxed_slice()) as *const u8
    };
    EmuHttpResponse {
        status: i32::from(received.status.as_u16()),
        headers,
        header_count,
        body,
        body_len,
        reason: reason.into_raw(),
        error: ptr::null(),
    }
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
        let prepared = match unsafe { prepare(http, request) } {
            Ok(prepared) => prepared,
            Err(message) => return failure(&message),
        };
        let outcome = http.runtime.block_on(async {
            let transfer = execute(&http.client, prepared);
            match http.timeout {
                Some(limit) => tokio::time::timeout(limit, transfer)
                    .await
                    .unwrap_or_else(|_| Err("the request timed out".to_owned())),
                None => transfer.await,
            }
        });
        match outcome {
            Ok(received) => into_response(received),
            Err(message) => failure(&message),
        }
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
