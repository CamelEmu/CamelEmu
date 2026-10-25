use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use rustls::crypto::ring::default_provider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ClientConfig, RootCertStore};

/// A TLS client configuration, optionally carrying a client certificate.
pub struct EmuTlsContext {
    #[allow(dead_code)]
    pub(crate) config: Arc<ClientConfig>,
}

unsafe fn pem_bytes<'a>(pem: *const c_char) -> Option<&'a [u8]> {
    (!pem.is_null()).then(|| unsafe { CStr::from_ptr(pem) }.to_bytes())
}

fn build(cert_pem: Option<&[u8]>, key_pem: Option<&[u8]>) -> Option<EmuTlsContext> {
    let roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let builder = ClientConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()
        .ok()?
        .with_root_certificates(roots);
    let config = match (cert_pem, key_pem) {
        (None, None) => builder.with_no_client_auth(),
        (Some(cert_pem), Some(key_pem)) => {
            let chain = CertificateDer::pem_slice_iter(cert_pem)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            if chain.is_empty() {
                return None;
            }
            let key = PrivateKeyDer::from_pem_slice(key_pem).ok()?;
            builder.with_client_auth_cert(chain, key).ok()?
        }
        _ => return None,
    };
    Some(EmuTlsContext {
        config: Arc::new(config),
    })
}

/// Builds a rustls client configuration that verifies servers against the
/// bundled Web PKI roots.
///
/// `cert_pem` and `key_pem` are NUL-terminated PEM strings for the client
/// certificate chain and its private key. Both must be given, or both NULL
/// for no client authentication. Returns NULL on failure.
///
/// # Safety
/// The strings must be NUL-terminated, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_crypto_tls_client_new(
    cert_pem: *const c_char,
    key_pem: *const c_char,
) -> *mut EmuTlsContext {
    catch_unwind(AssertUnwindSafe(|| {
        let (cert_pem, key_pem) = unsafe { (pem_bytes(cert_pem), pem_bytes(key_pem)) };
        build(cert_pem, key_pem)
    }))
    .ok()
    .flatten()
    .map_or(std::ptr::null_mut(), |context| Box::into_raw(Box::new(context)))
}

/// Releases a TLS context. Accepts NULL.
///
/// # Safety
/// `context` must come from `emu_crypto_tls_client_new` and must not be used
/// afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_crypto_tls_free(context: *mut EmuTlsContext) {
    if !context.is_null() {
        drop(unsafe { Box::from_raw(context) });
    }
}
