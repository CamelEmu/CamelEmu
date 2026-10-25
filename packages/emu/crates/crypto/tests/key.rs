mod common;

use common::*;
use emu_crypto::*;

fn verifies_with(data: &[u8]) -> bool {
    let key = Key::parse(data);
    if key.0.is_null() {
        return false;
    }
    let (token, issuer, audience) = (c(&sign(&valid_claims())), c(ISSUER), c(AUDIENCE));
    let status =
        unsafe { emu_crypto_jwt_verify_rs256(key.0, ptr(&token), ptr(&issuer), ptr(&audience)) };
    status == EMU_CRYPTO_JWT_OK
}

fn padded(der: &[u8], count: usize) -> Vec<u8> {
    let mut data = der.to_vec();
    data.extend(std::iter::repeat_n(count as u8, count));
    data
}

#[test]
fn parses_every_accepted_format() {
    for (name, data) in [
        ("SPKI PEM", PUBLIC_SPKI_PEM),
        ("PKCS#1 public PEM", PUBLIC_PKCS1_PEM),
        ("PKCS#1 private PEM", PRIVATE_PKCS1_PEM),
        ("PKCS#8 private PEM", PRIVATE_PKCS8_PEM),
        ("SPKI DER", PUBLIC_SPKI_DER),
        ("PKCS#1 public DER", PUBLIC_PKCS1_DER),
        ("PKCS#1 private DER", PRIVATE_PKCS1_DER),
        ("PKCS#8 private DER", PRIVATE_PKCS8_DER),
    ] {
        assert!(verifies_with(data), "{name}");
    }
}

#[test]
fn ignores_padding_after_a_der_element() {
    for (name, der) in [
        ("SPKI DER", PUBLIC_SPKI_DER),
        ("PKCS#1 public DER", PUBLIC_PKCS1_DER),
        ("PKCS#1 private DER", PRIVATE_PKCS1_DER),
        ("PKCS#8 private DER", PRIVATE_PKCS8_DER),
    ] {
        for count in [1, 7, 15, 16] {
            assert!(verifies_with(&padded(der, count)), "{name} + {count}");
        }
    }
}

#[test]
fn parses_pem_with_surrounding_whitespace() {
    let mut data = b"\n\n".to_vec();
    data.extend_from_slice(PUBLIC_SPKI_PEM);
    data.extend_from_slice(b"\r\n\r\n");
    assert!(verifies_with(&data));
}

#[test]
fn a_public_key_parsed_from_a_private_key_does_not_accept_another_signer() {
    let other = Key::parse(OTHER_PRIVATE_PEM);
    assert!(!other.0.is_null());
    let (token, issuer, audience) = (c(&sign(&valid_claims())), c(ISSUER), c(AUDIENCE));
    let status =
        unsafe { emu_crypto_jwt_verify_rs256(other.0, ptr(&token), ptr(&issuer), ptr(&audience)) };
    assert_eq!(status, EMU_CRYPTO_JWT_BAD_SIGNATURE);
}

#[test]
fn rejects_invalid_input() {
    assert!(Key::parse(b"").0.is_null());
    assert!(Key::parse(b"\0").0.is_null());
    assert!(Key::parse(b"not a key").0.is_null());
    assert!(Key::parse(&[0x30]).0.is_null());
    assert!(Key::parse(&[0x30, 0x82, 0xff, 0xff, 0x00]).0.is_null());
    assert!(Key::parse(&[0xff; 64]).0.is_null());
    assert!(Key::parse(b"-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----\n").0.is_null());
}

#[test]
fn rejects_truncated_keys() {
    for data in [PUBLIC_SPKI_DER, PRIVATE_PKCS8_DER] {
        for cut in [1, data.len() / 2, data.len() - 1] {
            assert!(Key::parse(&data[..cut]).0.is_null(), "cut {cut} of {}", data.len());
        }
    }
    for cut in [1, PUBLIC_SPKI_PEM.len() / 2, PUBLIC_SPKI_PEM.len() - 40] {
        assert!(Key::parse(&PUBLIC_SPKI_PEM[..cut]).0.is_null(), "PEM cut {cut}");
    }
}

#[test]
fn rejects_a_key_of_another_algorithm() {
    let der = padded(&[0x30, 0x03, 0x02, 0x01, 0x00], 3);
    assert!(Key::parse(&der).0.is_null());
}

#[test]
fn rejects_null_data_and_frees_null() {
    unsafe {
        assert!(emu_crypto_key_parse(std::ptr::null(), 16).is_null());
        emu_crypto_key_free(std::ptr::null_mut());
    }
}
