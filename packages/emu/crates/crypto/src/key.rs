use std::panic::{AssertUnwindSafe, catch_unwind};

use jsonwebtoken::DecodingKey;
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPublicKey};
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use rsa::{RsaPrivateKey, RsaPublicKey};

/// An RSA public key usable for signature verification.
///
/// A private key is accepted as input too; only its public half is kept.
pub struct EmuKeyHandle {
    pub(crate) decoding: DecodingKey,
}

/// Returns the first DER element of `data` and drops whatever follows it.
///
/// The client certificate of the console keeps the CBC padding after the DER
/// once it is decrypted, and strict parsers reject such trailing bytes.
fn first_der_element(data: &[u8]) -> &[u8] {
    if data.len() < 2 || data[0] != 0x30 {
        return data;
    }
    let (header, length) = match data[1] {
        n if n < 0x80 => (2, n as usize),
        0x81 if data.len() >= 3 => (3, data[2] as usize),
        0x82 if data.len() >= 4 => (4, u16::from_be_bytes([data[2], data[3]]) as usize),
        _ => return data,
    };
    data.get(..header + length).unwrap_or(data)
}

fn public_from_pem(pem: &str) -> Option<RsaPublicKey> {
    RsaPublicKey::from_public_key_pem(pem)
        .or_else(|_| RsaPublicKey::from_pkcs1_pem(pem))
        .ok()
        .or_else(|| {
            RsaPrivateKey::from_pkcs8_pem(pem)
                .or_else(|_| RsaPrivateKey::from_pkcs1_pem(pem))
                .ok()
                .map(|private| private.to_public_key())
        })
}

fn public_from_der(der: &[u8]) -> Option<RsaPublicKey> {
    let der = first_der_element(der);
    RsaPublicKey::from_public_key_der(der)
        .or_else(|_| RsaPublicKey::from_pkcs1_der(der))
        .ok()
        .or_else(|| {
            RsaPrivateKey::from_pkcs8_der(der)
                .or_else(|_| RsaPrivateKey::from_pkcs1_der(der))
                .ok()
                .map(|private| private.to_public_key())
        })
}

fn parse(data: &[u8]) -> Option<EmuKeyHandle> {
    let is_pem = data.windows(10).any(|window| window == b"-----BEGIN");
    let public = if is_pem {
        public_from_pem(std::str::from_utf8(data).ok()?.trim())?
    } else {
        public_from_der(data)?
    };
    let pkcs1 = public.to_pkcs1_der().ok()?;
    Some(EmuKeyHandle {
        decoding: DecodingKey::from_rsa_der(pkcs1.as_bytes()),
    })
}

/// Parses an RSA key given as PEM or as a single DER element.
///
/// Accepts SPKI and PKCS#1 public keys, and PKCS#8 and PKCS#1 private keys.
/// Bytes after a DER element are ignored. Returns NULL on failure.
///
/// # Safety
/// `data` must be valid for reads of `len` bytes, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_crypto_key_parse(data: *const u8, len: usize) -> *mut EmuKeyHandle {
    if data.is_null() || len == 0 {
        return std::ptr::null_mut();
    }
    let data = unsafe { std::slice::from_raw_parts(data, len) };
    catch_unwind(AssertUnwindSafe(|| parse(data)))
        .ok()
        .flatten()
        .map_or(std::ptr::null_mut(), |handle| Box::into_raw(Box::new(handle)))
}

/// Releases a key. Accepts NULL.
///
/// # Safety
/// `key` must come from `emu_crypto_key_parse` and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_crypto_key_free(key: *mut EmuKeyHandle) {
    if !key.is_null() {
        drop(unsafe { Box::from_raw(key) });
    }
}
