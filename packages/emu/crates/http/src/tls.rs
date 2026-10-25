//! TLS configuration for [`crate::emu_http_open`]: optional client certificate authentication and
//! optional server certificate verification, handed over as DER like the HTTP service of the
//! console does.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, ring};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};

pub struct EmuTlsContext {
    pub(crate) config: ClientConfig,
}

/// Accepts every server certificate. The handshake signatures are still checked, so only the trust
/// in the certificate chain is skipped.
#[derive(Debug)]
struct AcceptAnyServer(Arc<CryptoProvider>);

impl ServerCertVerifier for AcceptAnyServer {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Cuts a DER element off whatever follows it. OpenSSL's `d2i_*` parse one element and ignore the
/// rest, and the client certificate of the console comes out of a block cipher with its padding
/// still attached, which rustls and ring reject.
fn trim_der(mut der: Vec<u8>) -> Vec<u8> {
    let length = match der.get(1).copied() {
        Some(first) if first < 0x80 => Some(2 + usize::from(first)),
        Some(first) => {
            let count = usize::from(first & 0x7f);
            der.get(2..2 + count)
                .filter(|_| (1..=4).contains(&count))
                .map(|bytes| 2 + count + bytes.iter().fold(0, |acc, &b| (acc << 8) | usize::from(b)))
        }
        None => None,
    };
    if let Some(length) = length.filter(|&length| length <= der.len()) {
        der.truncate(length);
    }
    der
}

fn build(
    verify_server: bool,
    client_auth: Option<(Vec<u8>, Vec<u8>)>,
) -> Result<ClientConfig, Error> {
    let provider = Arc::new(ring::default_provider());
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?;
    let builder = if verify_server {
        let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        builder.with_root_certificates(roots)
    } else {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyServer(provider)))
    };
    match client_auth {
        Some((cert, key)) => {
            let key = PrivateKeyDer::try_from(trim_der(key))
                .map_err(|error| Error::General(error.to_string()))?;
            builder.with_client_auth_cert(vec![CertificateDer::from(trim_der(cert))], key)
        }
        None => Ok(builder.with_no_client_auth()),
    }
}

/// Creates a TLS context. `cert_der` and `key_der` are the DER encoded client certificate and its
/// private key; pass NULL for both to connect without client authentication. A non-zero
/// `verify_server` verifies the server against the bundled Mozilla roots, zero accepts any server.
/// Returns NULL when the certificate or the key is rejected.
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
            let (cert, key) = unsafe {
                (
                    std::slice::from_raw_parts(cert_der, cert_len),
                    std::slice::from_raw_parts(key_der, key_len),
                )
            };
            Some((cert.to_vec(), key.to_vec()))
        };
        match build(verify_server != 0, client_auth) {
            Ok(config) => Box::into_raw(Box::new(EmuTlsContext { config })),
            Err(_) => ptr::null_mut(),
        }
    }))
    .unwrap_or(ptr::null_mut())
}

/// # Safety
/// `tls` must be NULL or a context returned by [`emu_tls_context_new`] that has not been freed. A
/// context may be freed once the endpoints opened with it are closed, or earlier, as an endpoint
/// keeps its own copy of the configuration.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_tls_context_free(tls: *mut EmuTlsContext) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !tls.is_null() {
            drop(unsafe { Box::from_raw(tls) });
        }
    }));
}
