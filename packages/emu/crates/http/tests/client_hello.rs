//! The TLS handshake of the HTTP service has to look the way it did when httplib and LibreSSL sent
//! it, because the servers of the console only accept what it negotiated. The expected values are
//! the ClientHello that the default `TLS_client_method` context of LibreSSL 3.8.0 sent.

use std::ffi::CString;
use std::io::Read;
use std::net::TcpListener;
use std::ptr;
use std::sync::mpsc;
use std::thread;

use emu_http::{
    EmuHttpRequest, emu_http_close, emu_http_open, emu_http_request, emu_http_response_free,
    emu_tls_context_free, emu_tls_context_new,
};

const CIPHER_SUITES: [u16; 48] = [
    0x1302, 0x1303, 0x1301, 0xC030, 0xC02C, 0xC028, 0xC024, 0xC014, 0xC00A, 0x009F, 0x006B, 0x0039,
    0xCCA9, 0xCCA8, 0xCCAA, 0xFF85, 0x00C4, 0x0088, 0x0081, 0x009D, 0x003D, 0x0035, 0x00C0, 0x0084,
    0xC02F, 0xC02B, 0xC027, 0xC023, 0xC013, 0xC009, 0x009E, 0x0067, 0x0033, 0x00BE, 0x0045, 0x009C,
    0x003C, 0x002F, 0x00BA, 0x0041, 0xC011, 0xC007, 0x0005, 0xC012, 0xC008, 0x0016, 0x000A, 0x00FF,
];
const EXTENSIONS: [u16; 7] = [0, 10, 11, 13, 35, 43, 51];
const SIGNATURE_ALGORITHMS: [u16; 11] = [
    0x0806, 0x0601, 0x0603, 0x0805, 0x0501, 0x0503, 0x0804, 0x0401, 0x0403, 0x0201, 0x0203,
];
const SUPPORTED_GROUPS: [u16; 4] = [0x001D, 0x0017, 0x0018, 0x0019];
const SUPPORTED_VERSIONS: [u16; 4] = [0x0304, 0x0303, 0x0302, 0x0301];
const RECORD_VERSION: u16 = 0x0301;
const CLIENT_VERSION: u16 = 0x0303;

const SIGNATURE_ALGORITHMS_EXTENSION: u16 = 13;
const SUPPORTED_GROUPS_EXTENSION: u16 = 10;
const SUPPORTED_VERSIONS_EXTENSION: u16 = 43;

struct ClientHello {
    record_version: u16,
    client_version: u16,
    cipher_suites: Vec<u16>,
    extensions: Vec<(u16, Vec<u8>)>,
}

impl ClientHello {
    fn list(&self, kind: u16, skip: usize) -> Vec<u16> {
        let (_, body) = self
            .extensions
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .unwrap_or_else(|| panic!("extension {kind} is missing"));
        body[skip..]
            .chunks(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect()
    }
}

fn u16_at(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

fn parse(record: &[u8]) -> ClientHello {
    let hello = &record[5..];
    let mut offset = 4 + 2 + 32;
    offset += 1 + usize::from(hello[offset]);
    let length = usize::from(u16_at(hello, offset));
    let cipher_suites = (0..length)
        .step_by(2)
        .map(|i| u16_at(hello, offset + 2 + i))
        .collect();
    offset += 2 + length;
    offset += 1 + usize::from(hello[offset]);
    let end = offset + 2 + usize::from(u16_at(hello, offset));
    offset += 2;
    let mut extensions = Vec::new();
    while offset < end {
        let kind = u16_at(hello, offset);
        let length = usize::from(u16_at(hello, offset + 2));
        extensions.push((kind, hello[offset + 4..offset + 4 + length].to_vec()));
        offset += 4 + length;
    }
    ClientHello {
        record_version: u16_at(record, 1),
        client_version: u16_at(hello, 4),
        cipher_suites,
        extensions,
    }
}

/// Sends one request over TLS to a listener that only records the ClientHello.
fn capture_client_hello() -> ClientHello {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        let mut record = Vec::new();
        let mut chunk = [0u8; 4096];
        while record.len() < 5 || record.len() < 5 + usize::from(u16_at(&record, 3)) {
            let read = connection.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            record.extend_from_slice(&chunk[..read]);
        }
        sender.send(record).unwrap();
    });

    let host = CString::new(format!("https://localhost:{port}")).unwrap();
    let method = CString::new("GET").unwrap();
    let path = CString::new("/").unwrap();
    unsafe {
        let tls = emu_tls_context_new(0, ptr::null(), 0, ptr::null(), 0);
        let http = emu_http_open(host.as_ptr(), 5, tls);
        emu_tls_context_free(tls);
        assert!(!http.is_null());
        let request = EmuHttpRequest {
            method: method.as_ptr(),
            path: path.as_ptr(),
            headers: ptr::null_mut(),
            header_count: 0,
            body: ptr::null(),
            body_len: 0,
            provide_body: None,
            user: ptr::null_mut(),
            follow_redirects: 0,
        };
        let mut response = emu_http_request(http, &request);
        emu_http_response_free(&mut response);
        emu_http_close(http);
    }
    parse(&receiver.recv().unwrap())
}

#[test]
fn sends_the_client_hello_of_the_old_client() {
    let hello = capture_client_hello();

    assert_eq!(hello.record_version, RECORD_VERSION);
    assert_eq!(hello.client_version, CLIENT_VERSION);
    assert_eq!(hello.cipher_suites, CIPHER_SUITES);
    let mut kinds: Vec<u16> = hello.extensions.iter().map(|(kind, _)| *kind).collect();
    kinds.sort_unstable();
    assert_eq!(kinds, EXTENSIONS);
    assert_eq!(
        hello.list(SIGNATURE_ALGORITHMS_EXTENSION, 2),
        SIGNATURE_ALGORITHMS
    );
    assert_eq!(hello.list(SUPPORTED_GROUPS_EXTENSION, 2), SUPPORTED_GROUPS);
    assert_eq!(
        hello.list(SUPPORTED_VERSIONS_EXTENSION, 1),
        SUPPORTED_VERSIONS
    );
}
