use std::ffi::CString;

use emu_crypto::{EmuTlsContext, emu_crypto_tls_client_new, emu_crypto_tls_free};
use rcgen::generate_simple_self_signed;

struct Context(*mut EmuTlsContext);

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { emu_crypto_tls_free(self.0) };
    }
}

fn pem_pair() -> (CString, CString) {
    let pair = generate_simple_self_signed(vec!["client.invalid".into()]).unwrap();
    (
        CString::new(pair.cert.pem()).unwrap(),
        CString::new(pair.signing_key.serialize_pem()).unwrap(),
    )
}

fn create(cert: Option<&CString>, key: Option<&CString>) -> Context {
    let cert = cert.map_or(std::ptr::null(), |pem| pem.as_ptr());
    let key = key.map_or(std::ptr::null(), |pem| pem.as_ptr());
    Context(unsafe { emu_crypto_tls_client_new(cert, key) })
}

#[test]
fn builds_a_context_without_a_client_certificate() {
    assert!(!create(None, None).0.is_null());
}

#[test]
fn builds_a_context_with_a_client_certificate() {
    let (cert, key) = pem_pair();
    assert!(!create(Some(&cert), Some(&key)).0.is_null());
}

#[test]
fn rejects_a_certificate_without_its_key_or_the_reverse() {
    let (cert, key) = pem_pair();
    assert!(create(Some(&cert), None).0.is_null());
    assert!(create(None, Some(&key)).0.is_null());
}

#[test]
fn rejects_a_key_that_does_not_belong_to_the_certificate() {
    let (cert, _) = pem_pair();
    let (_, other_key) = pem_pair();
    assert!(create(Some(&cert), Some(&other_key)).0.is_null());
}

#[test]
fn rejects_invalid_pem() {
    let (cert, key) = pem_pair();
    let garbage = CString::new("not a pem").unwrap();
    assert!(create(Some(&garbage), Some(&key)).0.is_null());
    assert!(create(Some(&cert), Some(&garbage)).0.is_null());
    let empty = CString::new("").unwrap();
    assert!(create(Some(&empty), Some(&empty)).0.is_null());
}

#[test]
fn frees_null() {
    unsafe { emu_crypto_tls_free(std::ptr::null_mut()) };
}
