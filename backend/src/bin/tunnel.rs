//! `tunnel` bin — the reverse-tunnel CLI — PORT of `mock-tunnel` (§5.8, AC-49..52,
//! AC-S10). Opens one WS to `/ws/tunnel/{slug}`, replays each forwarded request to
//! a local server, and returns the response down the socket.
//!
//! Flags: `--port <p>` (local target, default 3000), `--endpoint <token>`,
//! `--secret <owner_secret>`, `--host <ws_base>` (default ws://localhost:8080).
//! Stdout follows the `cli.tty.*` contract (copy.md §5.12): connecting / bound /
//! per-request / 4401-stop (no retry) / 4409-exit / disconnect-backoff /
//! reconnected / shutdown.

use std::time::Duration;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::Message;

const WS_CLOSE_UNAUTHORIZED: u16 = 4401;
const WS_CLOSE_REBOUND: u16 = 4409;

struct Args {
    port: u16,
    endpoint: String,
    secret: String,
    host: String,
}

fn parse_args() -> Option<Args> {
    let mut port = 3000u16;
    let mut endpoint = String::new();
    let mut secret = String::new();
    let mut host = "ws://localhost:8080".to_string();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => port = it.next()?.parse().ok()?,
            "--endpoint" => endpoint = it.next()?,
            "--secret" => secret = it.next()?,
            "--host" => host = it.next()?,
            _ => {}
        }
    }
    if endpoint.is_empty() || secret.is_empty() {
        return None;
    }
    Some(Args {
        port,
        endpoint,
        secret,
        host,
    })
}

#[tokio::main]
async fn main() {
    let args = match parse_args() {
        Some(a) => a,
        None => {
            eprintln!("usage: tunnel --endpoint <token> --secret <owner_secret> [--port 3000] [--host ws://localhost:8080]");
            std::process::exit(2);
        }
    };

    let mut backoff = 1u64;
    loop {
        match run_once(&args).await {
            Outcome::Unauthorized => {
                println!("Authentication failed — your secret was rejected. Re-check --secret (it rotates each time you sign in). Stopping.");
                std::process::exit(1);
            }
            Outcome::Rebound => {
                println!("Disconnected — another tunnel took over this endpoint. Stopping.");
                std::process::exit(0);
            }
            Outcome::Disconnected => {
                println!("Connection lost. Reconnecting in {backoff}s…");
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(30);
            }
        }
    }
}

enum Outcome {
    Unauthorized,
    Rebound,
    Disconnected,
}

async fn run_once(args: &Args) -> Outcome {
    let url = format!(
        "{}/ws/tunnel/{}?cap={}",
        args.host.trim_end_matches('/'),
        args.endpoint,
        args.secret
    );
    let host_display = args.host.replace("ws://", "").replace("wss://", "");
    println!(
        "Connecting to {host_display} for endpoint {}…",
        args.endpoint
    );

    let ws = match tokio_tungstenite::connect_async(&url).await {
        Ok((ws, _)) => ws,
        Err(tokio_tungstenite::tungstenite::Error::Http(resp))
            if resp.status() == 401 || resp.status() == 403 =>
        {
            return Outcome::Unauthorized;
        }
        Err(_) => return Outcome::Disconnected,
    };

    let (mut sink, mut stream) = ws.split();
    let http = reqwest::Client::new();

    while let Some(msg) = stream.next().await {
        match msg {
            Ok(Message::Text(raw)) => {
                let frame: Value = match serde_json::from_str(&raw) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                match frame.get("t").and_then(Value::as_str) {
                    Some("bound") => {
                        println!(
                            "Tunnel up. Forwarding {} → http://localhost:{}",
                            args.endpoint, args.port
                        );
                    }
                    Some("req") => {
                        let resp = handle_request(&http, args.port, &frame).await;
                        if sink.send(Message::Text(resp.to_string())).await.is_err() {
                            return Outcome::Disconnected;
                        }
                    }
                    Some("ping") => {
                        let _ = sink
                            .send(Message::Text(json!({"t":"pong"}).to_string()))
                            .await;
                    }
                    Some("err") => {
                        // server-side rebound notice precedes a 4409 close.
                    }
                    _ => {}
                }
            }
            Ok(Message::Close(Some(cf))) => {
                let code: u16 = match cf.code {
                    CloseCode::Library(c) => c,
                    other => u16::from(other),
                };
                return match code {
                    WS_CLOSE_UNAUTHORIZED => Outcome::Unauthorized,
                    WS_CLOSE_REBOUND => Outcome::Rebound,
                    _ => Outcome::Disconnected,
                };
            }
            Ok(Message::Close(None)) => return Outcome::Disconnected,
            Ok(_) => {}
            Err(_) => return Outcome::Disconnected,
        }
    }
    Outcome::Disconnected
}

async fn handle_request(http: &reqwest::Client, port: u16, frame: &Value) -> Value {
    let id = frame.get("id").and_then(Value::as_u64).unwrap_or(0);
    let method = frame.get("method").and_then(Value::as_str).unwrap_or("GET");
    let path = frame.get("path").and_then(Value::as_str).unwrap_or("/");
    let body_b64 = frame.get("body_b64").and_then(Value::as_str).unwrap_or("");
    let body = base64::engine::general_purpose::STANDARD
        .decode(body_b64)
        .unwrap_or_default();
    let url = format!("http://localhost:{port}{path}");

    let m = reqwest::Method::from_bytes(method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut req = http.request(m, &url);
    if let Some(h) = frame.get("headers").and_then(Value::as_object) {
        for (k, v) in h {
            if let Some(vs) = v.as_str() {
                if !matches!(
                    k.to_ascii_lowercase().as_str(),
                    "host" | "content-length" | "connection"
                ) {
                    req = req.header(k, vs);
                }
            }
        }
    }
    if !body.is_empty() {
        req = req.body(body);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let mut headers = serde_json::Map::new();
            for (k, v) in resp.headers() {
                if let Ok(vs) = v.to_str() {
                    headers.insert(k.as_str().to_string(), json!(vs));
                }
            }
            let bytes = resp.bytes().await.unwrap_or_default();
            println!("{method} {path} → localhost:{port} ({status})");
            json!({
                "t": "res", "id": id, "status": status, "headers": headers,
                "body_b64": base64::engine::general_purpose::STANDARD.encode(&bytes),
            })
        }
        Err(_) => {
            println!("{method} {path} → localhost:{port} (502)");
            json!({"t":"err","id": id, "message": "local server unreachable"})
        }
    }
}
