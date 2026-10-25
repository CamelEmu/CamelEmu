#![allow(dead_code)]

use std::ffi::{CString, c_char};

use emu_crypto::{EmuKeyHandle, emu_crypto_key_free, emu_crypto_key_parse};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::{Value, json};

pub const PRIVATE_PKCS1_PEM: &[u8] = include_bytes!("../fixtures/key_pkcs1.pem");
pub const PRIVATE_PKCS8_PEM: &[u8] = include_bytes!("../fixtures/key_pkcs8.pem");
pub const PRIVATE_PKCS1_DER: &[u8] = include_bytes!("../fixtures/key_pkcs1.der");
pub const PRIVATE_PKCS8_DER: &[u8] = include_bytes!("../fixtures/key_pkcs8.der");
pub const PUBLIC_SPKI_PEM: &[u8] = include_bytes!("../fixtures/pub_spki.pem");
pub const PUBLIC_PKCS1_PEM: &[u8] = include_bytes!("../fixtures/pub_pkcs1.pem");
pub const PUBLIC_SPKI_DER: &[u8] = include_bytes!("../fixtures/pub_spki.der");
pub const PUBLIC_PKCS1_DER: &[u8] = include_bytes!("../fixtures/pub_pkcs1.der");
pub const OTHER_PRIVATE_PEM: &[u8] = include_bytes!("../fixtures/other_key_pkcs1.pem");
pub const OTHER_PUBLIC_SPKI_PEM: &[u8] = include_bytes!("../fixtures/other_pub_spki.pem");

pub const ISSUER: &str = "citra-core";
pub const AUDIENCE: &str = "external-uid";

pub const FAR_FUTURE: u64 = 4_102_444_800;
pub const LONG_PAST: u64 = 1_000_000_000;

/// A well-formed payload that every check accepts.
pub fn valid_claims() -> Value {
    json!({
        "iss": ISSUER,
        "aud": AUDIENCE,
        "iat": LONG_PAST,
        "exp": FAR_FUTURE,
        "jti": "token-id",
        "username": "camel",
        "displayName": "Camel",
        "avatarUrl": "https://example.invalid/avatar.png",
        "roles": ["moderator"],
    })
}

pub fn sign_with(algorithm: Algorithm, key: &EncodingKey, claims: &Value) -> String {
    encode(&Header::new(algorithm), claims, key).unwrap()
}

pub fn sign(claims: &Value) -> String {
    let key = EncodingKey::from_rsa_pem(PRIVATE_PKCS1_PEM).unwrap();
    sign_with(Algorithm::RS256, &key, claims)
}

pub fn without(claims: &Value, name: &str) -> Value {
    let mut claims = claims.clone();
    claims.as_object_mut().unwrap().remove(name);
    claims
}

pub fn with(claims: &Value, name: &str, value: Value) -> Value {
    let mut claims = claims.clone();
    claims[name] = value;
    claims
}

/// Owns a parsed key so that a failing assertion does not leak it.
pub struct Key(pub *mut EmuKeyHandle);

impl Key {
    pub fn parse(data: &[u8]) -> Key {
        Key(unsafe { emu_crypto_key_parse(data.as_ptr(), data.len()) })
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { emu_crypto_key_free(self.0) };
    }
}

pub fn c(text: &str) -> CString {
    CString::new(text).unwrap()
}

pub fn ptr(text: &CString) -> *const c_char {
    text.as_ptr()
}
