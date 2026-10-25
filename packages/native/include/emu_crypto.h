#ifndef EMU_CRYPTO_H
#define EMU_CRYPTO_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** An RSA public key usable for signature verification. */
typedef struct EmuKeyHandle EmuKeyHandle;

/** A TLS client configuration, optionally carrying a client certificate. */
typedef struct EmuTlsContext EmuTlsContext;

/** Statuses returned by emu_crypto_jwt_verify_rs256. */
enum {
    EMU_CRYPTO_JWT_OK = 0,
    EMU_CRYPTO_JWT_INVALID_ARGUMENT = 1,
    EMU_CRYPTO_JWT_MALFORMED = 2,
    EMU_CRYPTO_JWT_BAD_SIGNATURE = 3,
    EMU_CRYPTO_JWT_EXPIRED = 4,
    EMU_CRYPTO_JWT_NOT_YET_VALID = 5,
    EMU_CRYPTO_JWT_BAD_ISSUER = 6,
    EMU_CRYPTO_JWT_BAD_AUDIENCE = 7,
    EMU_CRYPTO_JWT_MISSING_CLAIM = 8,
    EMU_CRYPTO_JWT_BAD_ALGORITHM = 9,
};

/**
 * Parses an RSA key given as PEM or as a single DER element.
 *
 * SPKI and PKCS#1 public keys, and PKCS#8 and PKCS#1 private keys, are
 * accepted. Bytes after a DER element are ignored, because the client
 * certificate of the console keeps its padding after it is decrypted. Only the
 * public half of a private key is kept.
 *
 * Returns NULL on failure. Release the key with emu_crypto_key_free.
 */
EmuKeyHandle* emu_crypto_key_parse(const uint8_t* data, size_t len);

/** Releases a key. Accepts NULL. */
void emu_crypto_key_free(EmuKeyHandle* key);

/**
 * Verifies a JWT signed with RS256 against RFC 7519.
 *
 * Checks the signature, iss, aud, exp and nbf, and requires iat and jti. exp
 * is enforced only when present. Any other algorithm is rejected. All strings
 * are NUL-terminated.
 *
 * Returns an EMU_CRYPTO_JWT_* status.
 */
int32_t emu_crypto_jwt_verify_rs256(const EmuKeyHandle* key, const char* token, const char* issuer,
                                    const char* audience);

/**
 * Returns the payload of a token as a JSON string, after verifying it with the
 * same checks as emu_crypto_jwt_verify_rs256.
 *
 * Returns NULL when verification fails. out_len, when not NULL, receives the
 * length of the string without its terminator. Interpreting the claims is up
 * to the caller. Release the string with emu_crypto_string_free.
 */
char* emu_crypto_jwt_claims(const EmuKeyHandle* key, const char* token, const char* issuer,
                            const char* audience, size_t* out_len);

/** Releases a string returned by this library. Accepts NULL. */
void emu_crypto_string_free(char* string);

/**
 * Builds a TLS client configuration that verifies servers against the bundled
 * Web PKI roots.
 *
 * cert_pem and key_pem are NUL-terminated PEM strings holding the client
 * certificate chain and its private key. Pass both, or NULL for both to use no
 * client authentication. Returns NULL on failure or when the key does not belong
 * to the certificate. Release the context with emu_crypto_tls_free.
 */
EmuTlsContext* emu_crypto_tls_client_new(const char* cert_pem, const char* key_pem);

/** Releases a TLS context. Accepts NULL. */
void emu_crypto_tls_free(EmuTlsContext* context);

#ifdef __cplusplus
}
#endif

#endif
