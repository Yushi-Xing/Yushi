//! 本机 HTTP 替身：真实响应分块、长度边界及错误，不访问外部更新源。

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::{MAX_INDEX_BYTES, MAX_SIGNATURE_BYTES, download};
use crate::UpdateError;

fn receive_request(stream: &mut TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut headers = Vec::new();
    let mut byte = [0];
    while !headers.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        headers.push(byte[0]);
        assert!(headers.len() < 8192);
    }
}

fn fetch(url: &str, limit: usize) -> Result<Vec<u8>, UpdateError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let client = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap();
            download(&client, url, limit).await
        })
}

fn response(wire: Vec<u8>, limit: usize) -> Result<Vec<u8>, UpdateError> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/test", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        receive_request(&mut stream);
        let _ = stream.write_all(&wire);
    });
    let result = fetch(&url, limit);
    server.join().unwrap();
    result
}

#[test]
fn declared_lengths_empty_exact_and_oversized() {
    for size in [0, 1, 31, 32, 33, 4096] {
        let mut wire =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n")
                .into_bytes();
        wire.extend(vec![b'x'; size]);
        let result = response(wire, 32);
        if size <= 32 {
            assert_eq!(result.unwrap(), vec![b'x'; size]);
        } else {
            assert!(matches!(result, Err(UpdateError::TooLarge(32))));
        }
    }
}

#[test]
fn oversized_header_is_rejected_without_waiting_for_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/test", listener.local_addr().unwrap());
    let (release, wait) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        receive_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4096\r\n\r\n")
            .unwrap();
        wait.recv_timeout(Duration::from_secs(3)).unwrap();
    });
    let result = fetch(&url, 32);
    release.send(()).unwrap();
    server.join().unwrap();
    assert!(
        matches!(result, Err(UpdateError::TooLarge(32))),
        "{result:?}"
    );
}

#[test]
fn chunked_responses_limit_cumulative_bytes() {
    for size in [0, 31, 32, 33, 4096] {
        let mut wire =
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_vec();
        for chunk in vec![b'x'; size].chunks(7) {
            wire.extend(format!("{:x}\r\n", chunk.len()).as_bytes());
            wire.extend_from_slice(chunk);
            wire.extend_from_slice(b"\r\n");
        }
        wire.extend_from_slice(b"0\r\n\r\n");
        let result = response(wire, 32);
        if size <= 32 {
            assert_eq!(result.unwrap().len(), size);
        } else {
            assert!(matches!(result, Err(UpdateError::TooLarge(32))));
        }
    }
}

#[test]
fn unknown_length_and_utf8_are_bounded_by_bytes() {
    for size in [0, 32, 33] {
        let mut wire = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
        wire.extend(vec![b'x'; size]);
        let result = response(wire, 32);
        if size <= 32 {
            assert_eq!(result.unwrap().len(), size);
        } else {
            assert!(matches!(result, Err(UpdateError::TooLarge(32))));
        }
    }
    let wire = "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n你好"
        .as_bytes()
        .to_vec();
    assert!(matches!(
        response(wire.clone(), 5),
        Err(UpdateError::TooLarge(5))
    ));
    assert_eq!(response(wire, 6).unwrap(), "你好".as_bytes());
}

#[test]
fn bad_status_and_truncated_body_return_request_errors() {
    for status in [404, 429, 500] {
        assert!(matches!(
            response(
                format!("HTTP/1.1 {status} Failure\r\nContent-Length: 0\r\n\r\n").into_bytes(),
                32
            ),
            Err(UpdateError::Request(_))
        ));
    }
    assert!(matches!(
        response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nx".to_vec(),
            32
        ),
        Err(UpdateError::Request(_))
    ));
}

#[test]
fn production_index_and_signature_limits_allow_exact_boundary() {
    for limit in [MAX_SIGNATURE_BYTES, MAX_INDEX_BYTES] {
        let mut wire =
            format!("HTTP/1.1 200 OK\r\nContent-Length: {limit}\r\nConnection: close\r\n\r\n")
                .into_bytes();
        wire.extend(vec![b'x'; limit]);
        assert_eq!(response(wire, limit).unwrap().len(), limit);
    }
}

#[test]
fn chunked_overflow_is_rejected_before_the_stream_ends() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/test", listener.local_addr().unwrap());
    let (release, wait) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        receive_request(&mut stream);
        stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n21\r\nxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\r\n").unwrap();
        wait.recv_timeout(Duration::from_secs(3)).unwrap();
    });
    let result = fetch(&url, 32);
    release.send(()).unwrap();
    server.join().unwrap();
    assert!(
        matches!(result, Err(UpdateError::TooLarge(32))),
        "{result:?}"
    );
}
