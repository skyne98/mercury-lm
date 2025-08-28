use crate::models::{StreamEvent, ChatReq, Msg};
use std::{
    io::Read,
    sync::mpsc,
};

pub fn stream_chat(url: &str, model: String, msgs: Vec<Msg>, tx: mpsc::Sender<StreamEvent>) {
    let body = serde_json::to_string(&ChatReq {
        model,
        messages: msgs,
        stream: true,
        temperature: 0.7,
        max_tokens: 1024,
    })
    .unwrap();
    let url = url.to_string();
    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let post = client
            .post(format!("{url}/v1/chat/completions"))
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .body(body)
            .send();
        let mut resp = match post {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(format!("request failed: {e}")));
                let _ = tx.send(StreamEvent::Done);
                return;
            }
        };
        let mut buf = String::new();
        let mut chunk = [0u8; 8192];
        loop {
            match resp.read(&mut chunk) {
                Ok(0) => {
                    let _ = tx.send(StreamEvent::Done);
                    break;
                }
                Ok(n) => {
                    buf.push_str(&String::from_utf8_lossy(&chunk[..n]));
                    while let Some(idx) = buf.find("\n\n") {
                        let mut line = buf[..idx].trim().to_string();
                        buf = buf[(idx + 2)..].to_string();
                        if let Some(rest) = line.strip_prefix("data:") {
                            line = rest.trim().into();
                        }
                        if line == "[DONE]" {
                            let _ = tx.send(StreamEvent::Done);
                            return;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                            if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                                if tx.send(StreamEvent::Token(t.into())).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(format!("read failed: {e}")));
                    let _ = tx.send(StreamEvent::Done);
                    break;
                }
            }
        }
    });
}
