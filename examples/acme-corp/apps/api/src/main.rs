use std::{
    env,
    io::{Read, Write},
    net::TcpListener,
};

use shared::greeting;

fn main() {
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let bind = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&bind).expect("failed to bind TCP listener");
    println!("[api] listening on {bind} (RUST_LOG={rust_log})");

    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let mut buffer = [0u8; 4096];
        let _ = stream.read(&mut buffer);
        let body = format!("<pre>{}</pre>", greeting());
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    }
}