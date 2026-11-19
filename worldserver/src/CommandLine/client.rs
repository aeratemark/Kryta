//! Standalone local operator client for the world control interface.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::time::Duration;

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "init-credential" {
        return init_credential(std::path::Path::new(&args[1]));
    }
    if args.len() != 3 {
        return Err("usage: kryta-cli init-credential <file> | kryta-cli <port> <credential-file> <operation>".into());
    }
    let port = args[0].parse::<u16>().map_err(|_| "port must be a number")?;
    let credential = std::fs::read_to_string(&args[1]).map_err(|_| "cannot read credential file")?;
    let credential = credential.trim();
    if credential.len() < 32 || !credential.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("invalid credential file".into());
    }
    let body = serde_json::json!({"request_id":"cli", "operation": args[2], "arguments":{}}).to_string();
    let mut stream = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port))
        .map_err(|error| format!("cannot connect to world control: {error}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(30))).map_err(|error| error.to_string())?;
    let request = format!("POST /v1/operations HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {credential}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    stream.write_all(request.as_bytes()).map_err(|error| format!("request failed: {error}"))?;
    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|error| format!("response failed: {error}"))?;
    let (_, body) = response.split_once("\r\n\r\n").ok_or("invalid HTTP response")?;
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| "invalid JSON response")?;
    if value.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err(value.pointer("/error/message").and_then(|v| v.as_str()).unwrap_or("operation failed").into());
    }
    println!("{}", value.get("result").and_then(|v| v.as_str()).ok_or("missing result")?);
    Ok(())
}

#[cfg(windows)]
fn init_credential(path: &std::path::Path) -> Result<(), String> {
    use std::fs::OpenOptions;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|error| format!("cannot create credential file: {error}"))?;
    let script = r#"try { $ErrorActionPreference='Stop'; $p=$env:KRYTA_CONTROL_FILE; $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User; $a=Get-Acl -LiteralPath $p; $a.SetAccessRuleProtection($true,$false); $a.SetOwner($sid); $rule=New-Object Security.AccessControl.FileSystemAccessRule($sid,'FullControl','Allow'); $a.AddAccessRule($rule); Set-Acl -LiteralPath $p -AclObject $a; exit 0 } catch { exit 1 }"#;
    let status = std::process::Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("KRYTA_CONTROL_FILE", path).stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()
        .map_err(|_| "cannot set credential file permissions")?;
    if !status.success() { return Err("cannot restrict credential file permissions; empty file left in place".into()); }
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| "cannot generate a credential")?;
    let secret: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    file.write_all(secret.as_bytes()).map_err(|_| "cannot write credential")?;
    println!("Created restricted credential file {}.", path.display());
    Ok(())
}

#[cfg(not(windows))]
fn init_credential(_: &std::path::Path) -> Result<(), String> {
    Err("credential initialization is not implemented on this platform".into())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => { eprintln!("kryta-cli: {error}"); ExitCode::FAILURE }
    }
}
