//! TLS configuration for [`crate::emu_http_open`]: optional client certificate authentication and
//! optional server certificate verification, handed over as DER like the HTTP service of the
//! console does.
//!
//! The handshake goes through the same LibreSSL that the C++ core links, on a context with nothing
//! set but what the caller asked for, because the servers of the console only accept what the
//! console negotiated with the defaults of that library.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::ssl::{SslContext, SslContextBuilder, SslMethod, SslVerifyMode};
use openssl::x509::X509;

pub struct EmuTlsContext {
    pub(crate) context: SslContext,
}

/// The context of an endpoint that was opened without one: the defaults of the library, with the
/// server verified against its default trust store.
pub(crate) fn default_context() -> Result<SslContext, ErrorStack> {
    build(true, None)
}

/// `d2i_*` parse one DER element and ignore what follows it, such as the padding that the client
/// certificate of the console keeps after it is decrypted, and so do the parsers used here.
fn build(verify_server: bool, client_auth: Option<(&[u8], &[u8])>) -> Result<SslContext, ErrorStack> {
    let mut context = SslContextBuilder::new(SslMethod::tls_client())?;
    if verify_server {
        context.set_default_verify_paths()?;
        context.set_verify(SslVerifyMode::PEER);
    } else {
        context.set_verify(SslVerifyMode::NONE);
    }
    if let Some((cert, key)) = client_auth {
        let cert = X509::from_der(cert)?;
        let key = PKey::private_key_from_der(key)?;
        context.set_certificate(&cert)?;
        context.set_private_key(&key)?;
    }
    Ok(context.build())
}

/// Creates a TLS context. `cert_der` and `key_der` are the DER encoded client certificate and its
/// private key; pass NULL for both to connect without client authentication. A non-zero
/// `verify_server` verifies the server against the default trust store of the TLS library, zero
/// accepts any server. Returns NULL when the certificate or the key is rejected.
///
/// # Safety
/// `cert_der` and `key_der` must be NULL or readable for `cert_len` and `key_len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_tls_context_new(
    verify_server: i32,
    cert_der: *const u8,
    cert_len: usize,
    key_der: *const u8,
    key_len: usize,
) -> *mut EmuTlsContext {
    catch_unwind(AssertUnwindSafe(|| {
        let client_auth = if cert_der.is_null() || key_der.is_null() {
            None
        } else {
            Some(unsafe {
                (
                    std::slice::from_raw_parts(cert_der, cert_len),
                    std::slice::from_raw_parts(key_der, key_len),
                )
            })
        };
        match build(verify_server != 0, client_auth) {
            Ok(context) => Box::into_raw(Box::new(EmuTlsContext { context })),
            Err(_) => ptr::null_mut(),
        }
    }))
    .unwrap_or(ptr::null_mut())
}

/// # Safety
/// `tls` must be NULL or a context returned by [`emu_tls_context_new`] that has not been freed. A
/// context may be freed once the endpoints opened with it are closed, or earlier, as an endpoint
/// keeps its own reference to the configuration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_tls_context_free(tls: *mut EmuTlsContext) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !tls.is_null() {
            drop(unsafe { Box::from_raw(tls) });
        }
    }));
}
