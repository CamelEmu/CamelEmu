//! Requests through the C ABI against local servers: the redirect and body handling that this crate
//! implements on top of hyper, and the client certificate of the console, which comes out of a
//! block cipher with its padding still attached.

use std::ffi::{CStr, CString, c_void};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::ptr;
use std::sync::Arc;
use std::thread;

use emu_http::{
    EmuHttpHeader, EmuHttpRequest, EmuHttpResponse, emu_http_close, emu_http_open,
    emu_http_request, emu_http_response_free, emu_tls_context_free, emu_tls_context_new,
};
use openssl::asn1::Asn1Time;
use openssl::bn::BigNum;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::ssl::{SslAcceptor, SslMethod, SslVerifyMode};
use openssl::x509::{X509, X509NameBuilder};

struct Received {
    path: String,
    chunked: bool,
    body: Vec<u8>,
    echo: Option<String>,
}

fn read_request(reader: &mut BufReader<impl Read>) -> Option<Received> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let path = line.split_whitespace().nth(1)?.to_owned();
    let (mut length, mut chunked, mut echo) = (0usize, false, None);
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        let (name, value) = header.split_once(':')?;
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.parse().ok()?,
            "transfer-encoding" => chunked = value.eq_ignore_ascii_case("chunked"),
            "x-test" => echo = Some(value.to_owned()),
            _ => {}
        }
    }
    let mut body = Vec::new();
    if chunked {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size).ok()?;
            let size = usize::from_str_radix(size.trim(), 16).ok()?;
            let mut chunk = vec![0; size + 2];
            reader.read_exact(&mut chunk).ok()?;
            if size == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..size]);
        }
    } else {
        body.resize(length, 0);
        reader.read_exact(&mut body).ok()?;
    }
    Some(Received { path, chunked, body, echo })
}

fn respond(stream: &mut impl Write, request: &Received) {
    let (status, location, body): (&str, Option<&str>, Vec<u8>) = match request.path.as_str() {
        "/redirect" => ("302 Found", Some("/target"), Vec::new()),
        "/target" => ("200 OK", None, b"target".to_vec()),
        _ => ("200 OK", None, request.body.clone()),
    };
    let mut head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n", body.len());
    if let Some(location) = location {
        head += &format!("Location: {location}\r\n");
    }
    head += &format!("X-Echo: {}\r\n", request.echo.as_deref().unwrap_or(""));
    head += &format!("X-Transfer: {}\r\n\r\n", if request.chunked { "chunked" } else { "length" });
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(&body).unwrap();
}

fn serve_plain() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                while let Some(request) = read_request(&mut reader) {
                    respond(&mut stream, &request);
                }
            });
        }
    });
    port
}

struct Source {
    data: Vec<u8>,
    position: usize,
}

unsafe extern "C" fn provide(user: *mut c_void, buf: *mut u8, len: usize, eof: *mut i32) -> i32 {
    let source = unsafe { &mut *(user as *mut Source) };
    let count = len.min(3).min(source.data.len() - source.position);
    unsafe {
        ptr::copy_nonoverlapping(source.data.as_ptr().add(source.position), buf, count);
        *eof = i32::from(source.position + count == source.data.len());
    }
    source.position += count;
    count as i32
}

struct Answer {
    status: i32,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Answer {
    fn header(&self, name: &str) -> &str {
        self.headers
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map_or("", |(_, value)| value)
    }
}

fn send(
    host: &str,
    tls: *mut emu_http::EmuTlsContext,
    path: &str,
    method: &str,
    follow_redirects: bool,
    body: Option<(Vec<u8>, bool)>,
) -> Answer {
    let host = CString::new(host).unwrap();
    let path = CString::new(path).unwrap();
    let method = CString::new(method).unwrap();
    let test = (CString::new("x-test").unwrap(), CString::new("value").unwrap());
    let mut headers = [EmuHttpHeader {
        name: test.0.as_ptr(),
        value: test.1.as_ptr(),
    }];
    let mut source = Source {
        data: body.as_ref().map(|(data, _)| data.clone()).unwrap_or_default(),
        position: 0,
    };
    let known_length = body.as_ref().is_some_and(|(_, known)| *known);
    unsafe {
        let http = emu_http_open(host.as_ptr(), 5, tls);
        assert!(!http.is_null());
        let request = EmuHttpRequest {
            method: method.as_ptr(),
            path: path.as_ptr(),
            headers: headers.as_mut_ptr(),
            header_count: headers.len(),
            body: ptr::null(),
            body_len: if known_length { source.data.len() } else { 0 },
            provide_body: body.is_some().then_some(provide as _),
            user: &mut source as *mut Source as *mut c_void,
            follow_redirects: i32::from(follow_redirects),
        };
        let mut response: EmuHttpResponse = emu_http_request(http, &request);
        if !response.error.is_null() {
            panic!("{}", CStr::from_ptr(response.error).to_string_lossy());
        }
        let answer = Answer {
            status: response.status,
            headers: std::slice::from_raw_parts(response.headers, response.header_count)
                .iter()
                .map(|header| {
                    (
                        CStr::from_ptr(header.name).to_string_lossy().into_owned(),
                        CStr::from_ptr(header.value).to_string_lossy().into_owned(),
                    )
                })
                .collect(),
            body: if response.body.is_null() {
                Vec::new()
            } else {
                std::slice::from_raw_parts(response.body, response.body_len).to_vec()
            },
        };
        emu_http_response_free(&mut response);
        emu_http_close(http);
        answer
    }
}

#[test]
fn follows_redirects_when_asked_to() {
    let host = format!("http://127.0.0.1:{}", serve_plain());
    let answer = send(&host, ptr::null_mut(), "/redirect", "GET", true, None);
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, b"target");
    assert_eq!(answer.header("x-echo"), "value");
}

#[test]
fn leaves_redirects_alone_otherwise() {
    let host = format!("http://127.0.0.1:{}", serve_plain());
    let answer = send(&host, ptr::null_mut(), "/redirect", "GET", false, None);
    assert_eq!(answer.status, 302);
}

#[test]
fn sends_a_body_of_known_length_with_its_length() {
    let host = format!("http://127.0.0.1:{}", serve_plain());
    let answer = send(&host, ptr::null_mut(), "/echo", "POST", false, Some((b"0123456789".to_vec(), true)));
    assert_eq!(answer.body, b"0123456789");
    assert_eq!(answer.header("x-transfer"), "length");
}

#[test]
fn sends_a_body_of_unknown_length_chunked() {
    let host = format!("http://127.0.0.1:{}", serve_plain());
    let answer = send(&host, ptr::null_mut(), "/echo", "POST", false, Some((b"0123456789".to_vec(), false)));
    assert_eq!(answer.body, b"0123456789");
    assert_eq!(answer.header("x-transfer"), "chunked");
}

fn certificate() -> (X509, PKey<openssl::pkey::Private>) {
    let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", "localhost").unwrap();
    let name = name.build();
    let mut builder = X509::builder().unwrap();
    builder.set_version(2).unwrap();
    builder.set_serial_number(&BigNum::from_u32(1).unwrap().to_asn1_integer().unwrap()).unwrap();
    builder.set_subject_name(&name).unwrap();
    builder.set_issuer_name(&name).unwrap();
    builder.set_pubkey(&key).unwrap();
    builder.set_not_before(&Asn1Time::days_from_now(0).unwrap()).unwrap();
    builder.set_not_after(&Asn1Time::days_from_now(1).unwrap()).unwrap();
    builder.sign(&key, MessageDigest::sha256()).unwrap();
    (builder.build(), key)
}

#[test]
fn presents_a_client_certificate_that_still_has_its_padding() {
    let (server_cert, server_key) = certificate();
    let (client_cert, client_key) = certificate();

    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    acceptor.set_certificate(&server_cert).unwrap();
    acceptor.set_private_key(&server_key).unwrap();
    acceptor.set_verify_callback(
        SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT,
        |_, _| true,
    );
    let acceptor = Arc::new(acceptor.build());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let stream: TcpStream = stream;
        let mut stream = acceptor.accept(stream).unwrap();
        let presented = stream.ssl().peer_certificate().is_some();
        let mut reader = BufReader::new(&mut stream);
        let request = read_request(&mut reader).unwrap();
        assert_eq!(request.path, "/certificate");
        let body = if presented { "presented" } else { "missing" };
        let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        reader.get_mut().write_all(head.as_bytes()).unwrap();
    });

    let mut cert = client_cert.to_der().unwrap();
    let mut key = client_key.private_key_to_der().unwrap();
    cert.extend_from_slice(&[0x0B; 11]);
    key.extend_from_slice(&[0x05; 5]);
    let answer = unsafe {
        let tls = emu_tls_context_new(0, cert.as_ptr(), cert.len(), key.as_ptr(), key.len());
        assert!(!tls.is_null());
        let answer = send(&format!("https://localhost:{port}"), tls, "/certificate", "GET", false, None);
        emu_tls_context_free(tls);
        answer
    };
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body, b"presented");
}
