use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};

use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, Validation, decode};
use serde_json::Value;

use crate::key::EmuKeyHandle;

pub const EMU_CRYPTO_JWT_OK: i32 = 0;
pub const EMU_CRYPTO_JWT_INVALID_ARGUMENT: i32 = 1;
pub const EMU_CRYPTO_JWT_MALFORMED: i32 = 2;
pub const EMU_CRYPTO_JWT_BAD_SIGNATURE: i32 = 3;
pub const EMU_CRYPTO_JWT_EXPIRED: i32 = 4;
pub const EMU_CRYPTO_JWT_NOT_YET_VALID: i32 = 5;
pub const EMU_CRYPTO_JWT_BAD_ISSUER: i32 = 6;
pub const EMU_CRYPTO_JWT_BAD_AUDIENCE: i32 = 7;
pub const EMU_CRYPTO_JWT_MISSING_CLAIM: i32 = 8;
pub const EMU_CRYPTO_JWT_BAD_ALGORITHM: i32 = 9;

fn status_of(kind: &ErrorKind) -> i32 {
    match kind {
        ErrorKind::InvalidSignature => EMU_CRYPTO_JWT_BAD_SIGNATURE,
        ErrorKind::ExpiredSignature => EMU_CRYPTO_JWT_EXPIRED,
        ErrorKind::ImmatureSignature => EMU_CRYPTO_JWT_NOT_YET_VALID,
        ErrorKind::InvalidIssuer => EMU_CRYPTO_JWT_BAD_ISSUER,
        ErrorKind::InvalidAudience => EMU_CRYPTO_JWT_BAD_AUDIENCE,
        ErrorKind::MissingRequiredClaim(_) => EMU_CRYPTO_JWT_MISSING_CLAIM,
        ErrorKind::InvalidAlgorithm | ErrorKind::InvalidAlgorithmName => {
            EMU_CRYPTO_JWT_BAD_ALGORITHM
        }
        _ => EMU_CRYPTO_JWT_MALFORMED,
    }
}

/// Verifies an RS256 token and returns its payload.
///
/// `exp` is only enforced when present, and `iat` and `jti` must be present,
/// which is what the server issuing these tokens guarantees.
fn verify(key: &EmuKeyHandle, token: &str, issuer: &str, audience: &str) -> Result<Value, i32> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.leeway = 0;
    validation.validate_nbf = true;
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[audience]);
    validation.set_required_spec_claims(&["iss", "aud"]);

    let payload = decode::<Value>(token, &key.decoding, &validation)
        .map_err(|error| status_of(error.kind()))?
        .claims;
    if !payload.get("iat").is_some_and(Value::is_number) || payload.get("jti").is_none() {
        return Err(EMU_CRYPTO_JWT_MISSING_CLAIM);
    }
    Ok(payload)
}

struct Arguments<'a> {
    key: &'a EmuKeyHandle,
    token: &'a str,
    issuer: &'a str,
    audience: &'a str,
}

unsafe fn arguments<'a>(
    key: *const EmuKeyHandle,
    token: *const c_char,
    issuer: *const c_char,
    audience: *const c_char,
) -> Option<Arguments<'a>> {
    if key.is_null() || token.is_null() || issuer.is_null() || audience.is_null() {
        return None;
    }
    unsafe {
        Some(Arguments {
            key: &*key,
            token: CStr::from_ptr(token).to_str().ok()?,
            issuer: CStr::from_ptr(issuer).to_str().ok()?,
            audience: CStr::from_ptr(audience).to_str().ok()?,
        })
    }
}

/// Verifies a JWT signed with RS256 against RFC 7519.
///
/// Checks the signature, `iss`, `aud`, `exp` and `nbf`, and requires `iat`
/// and `jti`. Any other algorithm is rejected. Returns an
/// `EMU_CRYPTO_JWT_*` status.
///
/// # Safety
/// `key` must come from `emu_crypto_key_parse`, and the strings must be
/// NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_crypto_jwt_verify_rs256(
    key: *const EmuKeyHandle,
    token: *const c_char,
    issuer: *const c_char,
    audience: *const c_char,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(args) = (unsafe { arguments(key, token, issuer, audience) }) else {
            return EMU_CRYPTO_JWT_INVALID_ARGUMENT;
        };
        match verify(args.key, args.token, args.issuer, args.audience) {
            Ok(_) => EMU_CRYPTO_JWT_OK,
            Err(status) => status,
        }
    }))
    .unwrap_or(EMU_CRYPTO_JWT_INVALID_ARGUMENT)
}

/// Returns the payload of a token as a JSON string, after verifying it with
/// the same checks as `emu_crypto_jwt_verify_rs256`.
///
/// Returns NULL when verification fails. The string is released with
/// `emu_crypto_string_free`, and `out_len` (optional) receives its length.
///
/// # Safety
/// Same as `emu_crypto_jwt_verify_rs256`. `out_len` must be valid or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_crypto_jwt_claims(
    key: *const EmuKeyHandle,
    token: *const c_char,
    issuer: *const c_char,
    audience: *const c_char,
    out_len: *mut usize,
) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| {
        let args = unsafe { arguments(key, token, issuer, audience) }?;
        let payload = verify(args.key, args.token, args.issuer, args.audience).ok()?;
        let json = CString::new(serde_json::to_string(&payload).ok()?).ok()?;
        if !out_len.is_null() {
            unsafe { *out_len = json.as_bytes().len() };
        }
        Some(json.into_raw())
    }))
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// Releases a string returned by this library. Accepts NULL.
///
/// # Safety
/// `string` must come from this library and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_crypto_string_free(string: *mut c_char) {
    if !string.is_null() {
        drop(unsafe { CString::from_raw(string) });
    }
}
