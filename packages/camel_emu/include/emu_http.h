#ifndef EMU_HTTP_H
#define EMU_HTTP_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Opaque handle to one HTTP(S) endpoint. Requests made through it share the
 * underlying connection pool. Released with emu_http_close.
 */
typedef struct EmuHttp EmuHttp;

/** Opaque TLS configuration. Released with emu_tls_context_free. */
typedef struct EmuTlsContext EmuTlsContext;

/**
 * Creates a TLS configuration. `cert_der` and `key_der` are the DER encoded
 * client certificate and its private key; pass NULL for both to connect
 * without client authentication. A non-zero `verify_server` verifies the
 * server against the default trust store of the TLS library, zero accepts any
 * server. The handshake uses the TLS library of the core with its defaults.
 * Returns NULL when the certificate or the key is rejected.
 */
EmuTlsContext* emu_tls_context_new(int32_t verify_server, const uint8_t* cert_der, size_t cert_len,
                                   const uint8_t* key_der, size_t key_len);

/**
 * Releases a TLS configuration. NULL is accepted. Endpoints opened with it
 * keep their own reference, so it may be released right after emu_http_open.
 */
void emu_tls_context_free(EmuTlsContext* tls);

/**
 * Opens an endpoint. `host` is the base URL including the scheme, for example
 * "https://api.citra-emu.org"; requests append their path to it.
 * `timeout_seconds` bounds connecting and each whole request, and 0 disables
 * it. `tls` is used for https hosts, and NULL verifies the server against the
 * default trust store of the TLS library. Returns NULL when `host` is not an
 * http or https URL.
 */
EmuHttp* emu_http_open(const char* host, uint32_t timeout_seconds, EmuTlsContext* tls);

/** Releases an endpoint. NULL is accepted. */
void emu_http_close(EmuHttp* http);

typedef struct {
    const char* name;
    const char* value;
} EmuHttpHeader;

/**
 * Called to fetch the next part of a request body. Writes up to `len` bytes
 * into `buf`, sets `*eof` to non-zero once the body is complete, and returns
 * the number of bytes written, or a negative value to abort the request.
 */
typedef int32_t (*EmuHttpProvideBody)(void* user, uint8_t* buf, size_t len, int32_t* eof);

typedef struct {
    const char* method;
    const char* path;
    EmuHttpHeader* headers;
    size_t header_count;
    /** The whole body. Pass NULL to supply it through provide_body instead. */
    const uint8_t* body;
    /**
     * With `body`, its length. With provide_body, the total length when it is
     * known, or 0 to send the body with chunked transfer encoding.
     */
    size_t body_len;
    EmuHttpProvideBody provide_body;
    void* user;
    /** Non-zero to follow redirects. */
    int32_t follow_redirects;
} EmuHttpRequest;

typedef struct {
    /** The HTTP status code, or 0 when the transfer failed. */
    int32_t status;
    EmuHttpHeader* headers;
    size_t header_count;
    const uint8_t* body;
    size_t body_len;
    const char* reason;
    /** The standard error text of a failed transfer, otherwise NULL. */
    const char* error;
} EmuHttpResponse;

/**
 * Sends a request and blocks until the whole response has been received.
 * Header names in the response are lower case. The result must be released
 * with emu_http_response_free.
 */
EmuHttpResponse emu_http_request(EmuHttp* http, const EmuHttpRequest* request);

/** Releases a response, including its headers, body and strings. */
void emu_http_response_free(EmuHttpResponse* response);

#ifdef __cplusplus
}
#endif

#endif
