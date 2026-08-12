use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

pub fn spawn_json_server<F>(handler: F) -> u16
where
    F: Fn(&str) -> String + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().expect("clone"));
            let mut request_line = String::new();
            let _ = reader.read_line(&mut request_line);
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).expect("line") == 0 || line.trim().is_empty() {
                    break;
                }
                headers.push_str(&line);
            }
            let mut body = String::new();
            if let Some(len) = headers.lines().find_map(|l| {
                let lower = l.trim().to_lowercase();
                lower
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().to_string())
            }) {
                let len: usize = len.parse().unwrap_or(0);
                let mut buf = vec![0u8; len];
                let _ = reader.read_exact(&mut buf);
                body = String::from_utf8_lossy(&buf).into_owned();
            }
            let response_body = handler(&body);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{response_body}"
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

pub fn openai_response(content: &str) -> String {
    serde_json::json!({
        "choices": [{
            "message": { "content": content },
            "finish_reason": "stop"
        }],
        "usage": { "total_tokens": 10 }
    })
    .to_string()
}
