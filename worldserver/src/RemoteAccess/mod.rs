//! Local authenticated control for the world command queue.
//!
//! Reference: `src/server/apps/worldserver/ACSoap/ACSoap.cpp:81-140` at 30a36ec4d:
//! authenticate the caller, enqueue the command, and wait for world-thread completion.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc};
use std::time::Duration;

use crate::command_line::Request;

const MAX_REQUEST: u64 = 16 * 1024;

/// A running control listener, stopped and joined before the world update stops.
pub struct Server {
    stop: Arc<AtomicBool>,
    thread: std::thread::JoinHandle<()>,
    port: u16,
}

impl Server {
    /// Bind on loopback and begin accepting authenticated requests.
    pub fn start(port: u16, credential_file: &Path, timeout: Duration,
                 sender: mpsc::SyncSender<Request>) -> Result<Self, String> {
        let credential = read_credential(credential_file)?;
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
            .map_err(|error| format!("control listener cannot bind: {error}"))?;
        let port = listener.local_addr().map_err(|error| error.to_string())?.port();
        listener.set_nonblocking(true).map_err(|error| error.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let thread = std::thread::Builder::new().name("world-control".into()).spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve(stream, &credential, timeout, &sender),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => break,
                }
            }
        }).map_err(|error| format!("control thread cannot start: {error}"))?;
        Ok(Self { stop, thread, port })
    }

    /// The loopback port the listener actually bound, which a caller needs when it asked for `0`.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stop accepting requests and wait for the listener to exit.
    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.thread.join();
    }
}

fn read_credential(path: &Path) -> Result<String, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "control credential file is unavailable")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("control credential must be a regular file".into());
    }
    verify_permissions(path)?;
    let secret = std::fs::read_to_string(path).map_err(|_| "control credential cannot be read")?;
    let secret = secret.trim().to_owned();
    if secret.len() < 32 || !secret.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("control credential must contain at least 32 ASCII letters or digits".into());
    }
    Ok(secret)
}

#[cfg(windows)]
fn verify_permissions(path: &Path) -> Result<(), String> {
    // Reject every allow ACE except the process user, SYSTEM and local administrators. Failure
    // to inspect an identity or owner fails closed; the file is never made less restrictive here.
    let script = r#"try { $ErrorActionPreference='Stop'; $a=Get-Acl -LiteralPath $env:KRYTA_CONTROL_FILE; $me=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value; $owner=([Security.Principal.NTAccount]$a.Owner).Translate([Security.Principal.SecurityIdentifier]).Value; if ($owner -ne $me) { exit 1 }; foreach ($r in $a.Access) { if ($r.AccessControlType -eq 'Allow') { $s=$r.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value; if ($s -ne $me -and $s -ne 'S-1-5-18' -and $s -ne 'S-1-5-32-544') { exit 1 } } }; exit 0 } catch { exit 1 }"#;
    let result = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("KRYTA_CONTROL_FILE", path)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null()).status()
        .map_err(|_| "cannot verify control credential permissions")?;
    if result.success() { Ok(()) } else { Err("control credential permissions are not restricted to the service user, SYSTEM and administrators".into()) }
}

#[cfg(not(windows))]
fn verify_permissions(_: &Path) -> Result<(), String> {
    Err("control credential permission verification is not implemented on this platform".into())
}

fn serve(mut stream: TcpStream, credential: &str, timeout: Duration, sender: &mpsc::SyncSender<Request>) {
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let result = read_request(&mut stream, credential).and_then(|(id, operation)| {
        let (reply, answer) = mpsc::channel();
        sender.try_send(Request { line: format!("@{operation}"), reply })
            .map_err(|_| (503, "world queue unavailable".to_string()))?;
        let answer = answer.recv_timeout(timeout).map_err(|_| (503, "world response timed out; completion is unknown".to_string()))?;
        if answer.starts_with("unknown operation:") {
            Ok((404, serde_json::json!({"request_id": id, "ok": false, "error": {"code":"unknown_operation", "message":answer}})))
        } else {
            Ok((200, serde_json::json!({"request_id": id, "ok": true, "result":answer})))
        }
    });
    let (status, body) = result.unwrap_or_else(|(code, message)| (code, serde_json::json!({"ok":false,"error":{"code":"request_failed","message":message}})));
    let body = body.to_string();
    let reason = match status { 200 => "OK", 400 => "Bad Request", 401 => "Unauthorized", 404 => "Not Found", _ => "Service Unavailable" };
    let response = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    drain(stream);
}

/// Read and discard whatever the caller has not sent, up to the request limit.
///
/// A refused request is answered before its body is read, and Windows resets a socket closed with
/// unread input - which would destroy the answer the caller is about to read. Draining first makes the
/// refusal observable; the read timeout and the byte cap keep a caller that keeps sending bounded.
fn drain(mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(50)));
    let mut sink = [0u8; 4096];
    let mut read = 0u64;
    while read < MAX_REQUEST {
        match stream.read(&mut sink) {
            Ok(0) => break,
            Ok(count) => read += count as u64,
            Err(_) => break,
        }
    }
}

fn read_request(stream: &mut TcpStream, credential: &str) -> Result<(String, String), (u16, String)> {
    let mut data = Vec::new();
    let mut byte = [0u8; 1];
    while !data.ends_with(b"\r\n\r\n") && data.len() < MAX_REQUEST as usize {
        stream.read_exact(&mut byte).map_err(|_| (400, "incomplete request".into()))?;
        data.push(byte[0]);
    }
    let header = std::str::from_utf8(&data).map_err(|_| (400, "invalid headers".into()))?;
    let mut lines = header.split("\r\n");
    if lines.next() != Some("POST /v1/operations HTTP/1.1") { return Err((404, "unknown endpoint".into())); }
    let mut length = None;
    let mut authorized = false;
    for line in lines.filter(|line| !line.is_empty()) {
        let Some((name, value)) = line.split_once(':') else { return Err((400, "invalid header".into())); };
        if name.eq_ignore_ascii_case("content-length") {
            if length.is_some() { return Err((400, "duplicate content length".into())); }
            length = value.trim().parse::<usize>().ok();
        } else if name.eq_ignore_ascii_case("authorization") {
            authorized = value.trim().strip_prefix("Bearer ").is_some_and(|token| token == credential);
        }
    }
    if !authorized { return Err((401, "authentication failed".into())); }
    let length = length.filter(|length| *length <= MAX_REQUEST as usize - data.len())
        .ok_or_else(|| (400, "invalid content length".into()))?;
    let mut body = vec![0; length];
    stream.read_exact(&mut body).map_err(|_| (400, "incomplete body".into()))?;
    let value: serde_json::Value = serde_json::from_slice(&body).map_err(|_| (400, "invalid JSON".into()))?;
    let id = value.get("request_id").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).ok_or_else(|| (400, "request_id is required".into()))?;
    let operation = value.get("operation").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).ok_or_else(|| (400, "operation is required".into()))?;
    if value.get("arguments") != Some(&serde_json::json!({})) { return Err((400, "arguments must be an empty object".into())); }
    Ok((id.to_owned(), operation.to_owned()))
}

#[cfg(test)]
mod tests {
    //! The endpoint's four answers and its two failure answers, over a real loopback socket.
    //!
    //! The world is faked by a thread on the receiving end of the same queue the world update drains,
    //! so these prove the transport, the credential check and the queue contract without a database.

    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc::Receiver;

    const CREDENTIAL: &str = "0123456789abcdef0123456789abcdef";

    /// A connected pair: the side `serve` answers on, and the side the caller writes on.
    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let client = TcpStream::connect(address).unwrap();
        let (server, _) = listener.accept().unwrap();
        (server, client)
    }

    /// What a caller gets back: the status code and the parsed body.
    fn exchange(request: &str, capacity: usize, world: impl FnOnce(Receiver<Request>) + Send + 'static)
        -> (u16, serde_json::Value)
    {
        let (server, mut client) = pair();
        let (sender, receiver) = mpsc::sync_channel::<Request>(capacity);
        let world = std::thread::spawn(move || world(receiver));
        let serving = std::thread::spawn(move || serve(server, CREDENTIAL, Duration::from_millis(200), &sender));
        client.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        serving.join().unwrap();
        world.join().unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse().ok())
            .unwrap();
        (status, serde_json::from_str(body).unwrap())
    }

    /// A request body, well formed unless the caller says otherwise.
    fn body(operation: &str) -> String {
        serde_json::json!({"request_id": "t1", "operation": operation, "arguments": {}}).to_string()
    }

    /// A complete HTTP request with an optional bearer token.
    fn request(token: Option<&str>, body: &str) -> String {
        let auth = token.map(|token| format!("Authorization: Bearer {token}\r\n")).unwrap_or_default();
        format!("POST /v1/operations HTTP/1.1\r\nHost: localhost\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
    }

    #[test]
    fn a_registered_operation_answers_its_result_and_the_world_sees_the_operation() {
        let (status, body) = exchange(&request(Some(CREDENTIAL), &body("world.status")), 4, |receiver| {
            let request = receiver.recv().unwrap();
            assert_eq!(request.line, "@world.status", "the queue carries the operation, not the transport");
            request.reply.send("World 7: 0 map(s) loaded.".into()).unwrap();
        });
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["ok"], true);
        assert_eq!(body["result"], "World 7: 0 map(s) loaded.");
        assert_eq!(body["request_id"], "t1");
    }

    #[test]
    fn an_unknown_operation_is_not_found() {
        let (status, body) = exchange(&request(Some(CREDENTIAL), &body("nope")), 4, |receiver| {
            let request = receiver.recv().unwrap();
            request.reply.send("unknown operation: nope".into()).unwrap();
        });
        assert_eq!(status, 404, "{body}");
        assert_eq!(body["error"]["code"], "unknown_operation");
    }

    #[test]
    fn a_request_without_the_credential_is_refused_and_never_reaches_the_world() {
        let (status, body) = exchange(&request(None, &body("world.status")), 4, |receiver| {
            assert!(
                receiver.recv_timeout(Duration::from_millis(300)).is_err(),
                "an unauthenticated request must not be queued"
            );
        });
        assert_eq!(status, 401, "{body}");
        assert_eq!(body["error"]["code"], "request_failed");
    }

    #[test]
    fn a_request_with_the_wrong_credential_is_refused() {
        let (status, _) = exchange(&request(Some("ffffffffffffffffffffffffffffffff"), &body("world.status")), 4, |receiver| {
            assert!(
                receiver.recv_timeout(Duration::from_millis(300)).is_err(),
                "a wrongly authenticated request must not be queued"
            );
        });
        assert_eq!(status, 401);
    }

    #[test]
    fn a_body_that_is_not_the_contract_is_rejected() {
        let not_queued = |receiver: Receiver<Request>| {
            assert!(
                receiver.recv_timeout(Duration::from_millis(300)).is_err(),
                "an invalid body must not be queued"
            );
        };
        let (status, body) = exchange(&request(Some(CREDENTIAL), "{\"operation\":\"world.status\"}"), 4, not_queued);
        assert_eq!(status, 400, "{body}");
        let (status, _) = exchange(&request(Some(CREDENTIAL), "not json"), 4, not_queued);
        assert_eq!(status, 400);
        let arguments = serde_json::json!({"request_id": "t1", "operation": "world.status", "arguments": {"path": "/"}}).to_string();
        let (status, _) = exchange(&request(Some(CREDENTIAL), &arguments), 4, not_queued);
        assert_eq!(status, 400);
    }

    #[test]
    fn a_full_queue_is_reported_as_unavailable() {
        // A rendezvous channel with no receiver blocked in `recv` cannot accept, which is a full queue.
        let (status, body) = exchange(&request(Some(CREDENTIAL), &body("world.status")), 0, |_receiver| {
            std::thread::sleep(Duration::from_millis(50));
        });
        assert_eq!(status, 503, "{body}");
        assert_eq!(body["error"]["message"], "world queue unavailable");
    }

    #[test]
    fn a_world_that_does_not_answer_is_a_timeout_naming_unknown_completion() {
        let (status, body) = exchange(&request(Some(CREDENTIAL), &body("world.status")), 1, |receiver| {
            let request = receiver.recv().unwrap();
            drop(request.reply);
        });
        assert_eq!(status, 503, "{body}");
        assert_eq!(body["error"]["message"], "world response timed out; completion is unknown");
    }

    #[test]
    fn a_missing_credential_file_is_refused_before_anything_listens() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let error = Server::start(0, Path::new("no-such-credential"), Duration::from_secs(1), sender)
            .err()
            .expect("a missing credential file must refuse the listener");
        assert!(error.contains("credential file is unavailable"), "{error}");
    }
}
