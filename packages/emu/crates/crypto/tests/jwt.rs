mod common;

use std::ffi::CStr;

use common::*;
use emu_crypto::*;
use jsonwebtoken::{Algorithm, EncodingKey};
use serde_json::{Value, json};

fn verify_with(key: &Key, token: &str) -> i32 {
    verify_for(key, token, ISSUER, AUDIENCE)
}

fn verify_for(key: &Key, token: &str, issuer: &str, audience: &str) -> i32 {
    let (token, issuer, audience) = (c(token), c(issuer), c(audience));
    unsafe { emu_crypto_jwt_verify_rs256(key.0, ptr(&token), ptr(&issuer), ptr(&audience)) }
}

fn verify(token: &str) -> i32 {
    verify_with(&Key::parse(PUBLIC_SPKI_PEM), token)
}

fn claims_of(token: &str) -> Option<Value> {
    let key = Key::parse(PUBLIC_SPKI_PEM);
    let (token, issuer, audience) = (c(token), c(ISSUER), c(AUDIENCE));
    let mut len = 0;
    let json = unsafe {
        emu_crypto_jwt_claims(key.0, ptr(&token), ptr(&issuer), ptr(&audience), &mut len)
    };
    if json.is_null() {
        return None;
    }
    let text = unsafe { CStr::from_ptr(json) }.to_str().unwrap().to_owned();
    assert_eq!(text.len(), len);
    unsafe { emu_crypto_string_free(json) };
    Some(serde_json::from_str(&text).unwrap())
}

#[test]
fn accepts_a_valid_token() {
    assert_eq!(verify(&sign(&valid_claims())), EMU_CRYPTO_JWT_OK);
}

#[test]
fn accepts_a_token_without_exp() {
    assert_eq!(verify(&sign(&without(&valid_claims(), "exp"))), EMU_CRYPTO_JWT_OK);
}

#[test]
fn accepts_an_audience_given_as_an_array() {
    let claims = with(&valid_claims(), "aud", json!(["other", AUDIENCE]));
    assert_eq!(verify(&sign(&claims)), EMU_CRYPTO_JWT_OK);
}

#[test]
fn rejects_a_tampered_payload() {
    let token = sign(&valid_claims());
    let mut parts: Vec<&str> = token.split('.').collect();
    let forged = sign(&with(&valid_claims(), "roles", json!(["moderator", "admin"])));
    parts[1] = forged.split('.').nth(1).unwrap();
    assert_eq!(verify(&parts.join(".")), EMU_CRYPTO_JWT_BAD_SIGNATURE);
}

#[test]
fn rejects_a_tampered_signature() {
    let token = sign(&valid_claims());
    let (head, signature) = token.rsplit_once('.').unwrap();
    let mut signature = signature.to_owned().into_bytes();
    signature[10] = if signature[10] == b'A' { b'B' } else { b'A' };
    let forged = format!("{head}.{}", String::from_utf8(signature).unwrap());
    assert_ne!(verify(&forged), EMU_CRYPTO_JWT_OK);
}

#[test]
fn rejects_a_token_signed_by_another_key() {
    let other = EncodingKey::from_rsa_pem(OTHER_PRIVATE_PEM).unwrap();
    let token = sign_with(Algorithm::RS256, &other, &valid_claims());
    assert_eq!(verify(&token), EMU_CRYPTO_JWT_BAD_SIGNATURE);
}

#[test]
fn rejects_the_wrong_public_key() {
    let key = Key::parse(OTHER_PUBLIC_SPKI_PEM);
    assert_eq!(
        verify_with(&key, &sign(&valid_claims())),
        EMU_CRYPTO_JWT_BAD_SIGNATURE
    );
}

#[test]
fn rejects_a_wrong_issuer() {
    let token = sign(&valid_claims());
    let key = Key::parse(PUBLIC_SPKI_PEM);
    assert_eq!(
        verify_for(&key, &token, "someone-else", AUDIENCE),
        EMU_CRYPTO_JWT_BAD_ISSUER
    );
    let claims = with(&valid_claims(), "iss", json!("someone-else"));
    assert_eq!(verify(&sign(&claims)), EMU_CRYPTO_JWT_BAD_ISSUER);
}

#[test]
fn rejects_a_wrong_audience() {
    let token = sign(&valid_claims());
    let key = Key::parse(PUBLIC_SPKI_PEM);
    assert_eq!(
        verify_for(&key, &token, ISSUER, "external-other"),
        EMU_CRYPTO_JWT_BAD_AUDIENCE
    );
}

#[test]
fn rejects_a_missing_issuer_or_audience() {
    assert_ne!(verify(&sign(&without(&valid_claims(), "iss"))), EMU_CRYPTO_JWT_OK);
    assert_ne!(verify(&sign(&without(&valid_claims(), "aud"))), EMU_CRYPTO_JWT_OK);
}

#[test]
fn rejects_an_expired_token() {
    let claims = with(&valid_claims(), "exp", json!(LONG_PAST));
    assert_eq!(verify(&sign(&claims)), EMU_CRYPTO_JWT_EXPIRED);
}

#[test]
fn rejects_a_token_that_is_not_valid_yet() {
    let claims = with(&valid_claims(), "nbf", json!(FAR_FUTURE));
    assert_eq!(verify(&sign(&claims)), EMU_CRYPTO_JWT_NOT_YET_VALID);
}

#[test]
fn rejects_a_token_without_iat_or_jti() {
    assert_eq!(
        verify(&sign(&without(&valid_claims(), "iat"))),
        EMU_CRYPTO_JWT_MISSING_CLAIM
    );
    assert_eq!(
        verify(&sign(&without(&valid_claims(), "jti"))),
        EMU_CRYPTO_JWT_MISSING_CLAIM
    );
    let claims = with(&valid_claims(), "iat", json!("yesterday"));
    assert_eq!(verify(&sign(&claims)), EMU_CRYPTO_JWT_MISSING_CLAIM);
}

#[test]
fn rejects_an_unsigned_token() {
    let token = sign(&valid_claims());
    let payload = token.split('.').nth(1).unwrap();
    let header = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0";
    assert_ne!(verify(&format!("{header}.{payload}.")), EMU_CRYPTO_JWT_OK);
    assert_ne!(verify(&format!("{header}.{payload}")), EMU_CRYPTO_JWT_OK);
}

#[test]
fn rejects_hs256_signed_with_the_public_key() {
    let secret = EncodingKey::from_secret(PUBLIC_SPKI_PEM);
    let token = sign_with(Algorithm::HS256, &secret, &valid_claims());
    assert_ne!(verify(&token), EMU_CRYPTO_JWT_OK);
    let secret = EncodingKey::from_secret(PUBLIC_PKCS1_DER);
    let token = sign_with(Algorithm::HS256, &secret, &valid_claims());
    assert_ne!(verify(&token), EMU_CRYPTO_JWT_OK);
}

#[test]
fn rejects_other_rsa_algorithms() {
    let key = EncodingKey::from_rsa_pem(PRIVATE_PKCS1_PEM).unwrap();
    for algorithm in [Algorithm::RS384, Algorithm::RS512, Algorithm::PS256] {
        let token = sign_with(algorithm, &key, &valid_claims());
        assert_eq!(verify(&token), EMU_CRYPTO_JWT_BAD_ALGORITHM, "{algorithm:?}");
    }
}

#[test]
fn rejects_malformed_tokens() {
    for token in [
        "",
        ".",
        "..",
        "a.b",
        "a.b.c",
        "a.b.c.d",
        "!!!.!!!.!!!",
        "not a token",
    ] {
        assert_ne!(verify(token), EMU_CRYPTO_JWT_OK, "{token:?}");
    }
    let token = sign(&valid_claims());
    assert_ne!(verify(&token[..token.len() / 2]), EMU_CRYPTO_JWT_OK);
}

#[test]
fn rejects_null_arguments() {
    let key = Key::parse(PUBLIC_SPKI_PEM);
    let (token, issuer, audience) = (c(&sign(&valid_claims())), c(ISSUER), c(AUDIENCE));
    unsafe {
        let null = std::ptr::null();
        let invalid = EMU_CRYPTO_JWT_INVALID_ARGUMENT;
        assert_eq!(
            emu_crypto_jwt_verify_rs256(std::ptr::null(), ptr(&token), ptr(&issuer), ptr(&audience)),
            invalid
        );
        assert_eq!(
            emu_crypto_jwt_verify_rs256(key.0, null, ptr(&issuer), ptr(&audience)),
            invalid
        );
        assert_eq!(
            emu_crypto_jwt_verify_rs256(key.0, ptr(&token), null, ptr(&audience)),
            invalid
        );
        assert_eq!(
            emu_crypto_jwt_verify_rs256(key.0, ptr(&token), ptr(&issuer), null),
            invalid
        );
        assert!(
            emu_crypto_jwt_claims(key.0, null, ptr(&issuer), ptr(&audience), std::ptr::null_mut())
                .is_null()
        );
        emu_crypto_string_free(std::ptr::null_mut());
    }
}

#[test]
fn claims_return_the_payload_of_a_verified_token() {
    let claims = claims_of(&sign(&valid_claims())).unwrap();
    assert_eq!(claims, valid_claims());
    assert_eq!(claims["username"], "camel");
    assert_eq!(claims["roles"], json!(["moderator"]));
}

#[test]
fn claims_are_withheld_when_verification_fails() {
    let other = EncodingKey::from_rsa_pem(OTHER_PRIVATE_PEM).unwrap();
    assert!(claims_of(&sign_with(Algorithm::RS256, &other, &valid_claims())).is_none());
    assert!(claims_of(&sign(&with(&valid_claims(), "exp", json!(LONG_PAST)))).is_none());
    assert!(claims_of("garbage").is_none());
}
