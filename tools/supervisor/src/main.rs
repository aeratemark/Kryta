//! The stack: one command that starts this machine's programs and stops them together.
//!
//! What it runs is data, not code: `stack.json` lists the services - a program, its arguments and
//! the port it binds - and the supervisor starts each one, prints its output under its own name,
//! waits for its READY line, and keeps them alive together. Ctrl+C stops them all; a service that
//! dies on its own stops the rest and the supervisor answers that service's exit code.
//!
//! This is the tree's own arrangement, taken from ASURA's `asura-supervisor` (one command for a
//! stack that is otherwise several windows) rather than from the reference, which has no such
//! program. What it does *not* do yet, and says so: no restart policy, no health checks beyond the
//! READY line, and no control channel.
//!
//! Run it from the repository root:
//!
//! ```text
//! cargo run -p kryta-supervisor                      # start the stack; Ctrl+C stops it
//! cargo run -p kryta-supervisor -- --check           # start, wait for every READY, stop, exit
//! cargo run -p kryta-supervisor -- -c tools/supervisor/stack.json
//! ```

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::configuration::json::Document;

/// Where the list lives, relative to the repository root.
const DEFAULT_STACK: &str = "tools/supervisor/stack.json";
/// How long a service may take to print its READY line.
const READY_TIMEOUT: Duration = Duration::from_secs(60);

/// One service, as `stack.json` states it.
#[derive(Debug, Clone)]
struct Service {
    /// What the supervisor calls it in the log's prefix.
    name: String,
    /// The cargo package's binary name, built under `target/debug/`.
    program: String,
    /// Its arguments, verbatim.
    args: Vec<String>,
    /// The port it binds, for the up-report.
    port: u16,
}

/// Read the list.
fn load(path: &std::path::Path) -> Result<Vec<Service>, String> {
    let document = Document::read(path).map_err(|error| error.to_string())?;
    let records = document.records("services")?;
    let mut services = Vec::with_capacity(records.len());
    for record in records {
        let port: u16 = record
            .required("port")?
            .parse()
            .map_err(|_| format!("{}: 'port' is not a number", record.at()))?;
        services.push(Service {
            name: record.required("name")?,
            program: record.required("program")?,
            args: record.strings("args")?.unwrap_or_default(),
            port,
        });
    }
    Ok(services)
}

/// The built binary of a service, beside the repository root.
fn exe_of(program: &str) -> std::path::PathBuf {
    let root = std::env::current_dir().expect("the working directory");
    let name = if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_string()
    };
    root.join("target").join("debug").join(name)
}

/// Build a service's program before it starts, so the one command always runs the code that is in the
/// tree - not a binary left over from the last build. Cargo is incremental: a tree that has not
/// changed builds in a fraction of a second, and a tree that has changed says so instead of being
/// quietly ignored. The build is cargo's own, printing its own progress to this console.
fn ensure_built(program: &str) -> Result<(), String> {
    let exe = exe_of(program);
    if !exe.exists() {
        println!("{program} is not built yet; building it now (cargo build -p {program})...");
    }
    let status = Command::new("cargo")
        .args(["build", "-p", program])
        .status()
        .map_err(|error| format!("cannot run cargo build for {program}: {error}"))?;
    if exe.exists() {
        return Ok(());
    }
    Err(format!(
        "{program} did not build (cargo answered {status}); `cargo build -p {program}` says why"
    ))
}

/// Start one service, and hand every line it prints to the supervisor.
fn start(service: &Service, lines: Sender<(String, String)>) -> Result<Child, String> {
    ensure_built(&service.program)?;
    let exe = exe_of(&service.program);
    let mut child = Command::new(&exe)
        .args(&service.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", service.name))?;
    let name = service.name.clone();
    let stdout = child.stdout.take().expect("the program's stdout is piped");
    let stderr = child.stderr.take().expect("the program's stderr is piped");
    let streams: [Box<dyn std::io::Read + Send>; 2] = [Box::new(stdout), Box::new(stderr)];
    for stream in streams {
        let lines = lines.clone();
        let name = name.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else {
                    return;
                };
                if lines.send((name.clone(), line)).is_err() {
                    return;
                }
            }
        });
    }
    Ok(child)
}

/// Print one line under its service's name.
fn report(name: &str, line: &str) {
    println!("{name:>6} | {line}");
}

/// Start every service and wait for the ones that have not said READY yet.
///
/// Answers whether each one's READY line was seen. Every line a service prints is printed here, so
/// a service that fails says why through its own words.
fn wait_ready(
    services: &[Service],
    ready: &mut [bool],
    children: &mut [(Service, Child)],
    lines: &Receiver<(String, String)>,
) -> bool {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match lines.recv_timeout(remaining.min(Duration::from_millis(200))) {
            Ok((name, line)) => {
                report(&name, &line);
                if line.contains("READY: listening on")
                    && let Some(index) = services.iter().position(|service| service.name == name)
                {
                    ready[index] = true;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return ready.iter().all(|up| *up),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }

        // A service that died while starting is reported with its own lines above; the others are
        // stopped so the whole command fails as one thing.
        for (service, child) in children.iter_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                report(&service.name, &format!("exited with {status}"));
                return false;
            }
        }
        if ready.iter().all(|up| *up) {
            return true;
        }
        if Instant::now() >= deadline {
            for (index, service) in services.iter().enumerate() {
                if !ready[index] {
                    report(&service.name, &format!("no READY line within {READY_TIMEOUT:?}"));
                }
            }
            return false;
        }
    }
}

/// Stop every child: kill it, then wait a moment for it to be gone.
fn stop_all(children: &mut [(Service, Child)]) {
    for (_, child) in children.iter_mut() {
        let _ = child.kill();
    }
    let deadline = Instant::now() + READY_TIMEOUT.min(Duration::from_secs(10));
    while Instant::now() < deadline {
        let mut running = 0usize;
        for (_, child) in children.iter_mut() {
            if matches!(child.try_wait(), Ok(None)) {
                running += 1;
            }
        }
        if running == 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn usage() {
    println!("Allowed options:");
    println!("  -h [ --help ]             print usage message");
    println!("  -c [ --config ] arg       use <arg> as the service list (default: {DEFAULT_STACK})");
    println!("  --check                   start, wait for every READY line, stop, and say so");
}


fn main() -> std::process::ExitCode {
    let mut config_path = std::path::PathBuf::from(DEFAULT_STACK);
    let mut check_only = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => {
                usage();
                return std::process::ExitCode::SUCCESS;
            }
            "-c" | "--config" => {
                let Some(value) = arguments.next() else {
                    eprintln!("--config needs a path");
                    return std::process::ExitCode::FAILURE;
                };
                config_path = std::path::PathBuf::from(value);
            }
            "--check" => check_only = true,
            other => {
                eprintln!("unknown option: {other}");
                usage();
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    let services = match load(&config_path) {
        Ok(services) => services,
        Err(error) => {
            eprintln!("the service list cannot be read: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = Arc::clone(&stop);
        let _ = ctrlc::set_handler(move || stop.store(true, Ordering::Relaxed));
    }

    let (lines, received) = channel();
    let mut children: Vec<(Service, Child)> = Vec::with_capacity(services.len());
    for service in &services {
        match start(service, lines.clone()) {
            Ok(child) => children.push((service.clone(), child)),
            Err(error) => {
                eprintln!("{error}");
                stop_all(&mut children);
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    let mut ready = vec![false; services.len()];
    let mut all_up = wait_ready(&services, &mut ready, &mut children, &received);
    if all_up {
        let list: Vec<String> = services
            .iter()
            .map(|service| format!("{} on {}", service.name, service.port))
            .collect();
        println!(
            "the stack is up: {} - {}",
            list.join(", "),
            if check_only {
                "checked, stopping now"
            } else {
                "Ctrl+C stops it"
            }
        );
    }

    if check_only || !all_up {
        stop_all(&mut children);
        for (name, line) in received.try_iter() {
            report(&name, &line);
        }
        return if all_up {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::FAILURE
        };
    }

    // Supervise: Ctrl+C stops everything; a service that dies stops the rest.
    loop {
        for (name, line) in received.try_iter() {
            report(&name, &line);
        }
        for (service, child) in children.iter_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                report(&service.name, &format!("exited with {status}; stopping the rest"));
                all_up = false;
                break;
            }
        }
        if !all_up || stop.load(Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    stop_all(&mut children);
    std::process::ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_stack(text: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "kryta-stack-{}-{}.json",
            std::process::id(),
            text.len()
        ));
        std::fs::write(&path, text).expect("the stack file is written");
        path
    }

    #[test]
    fn the_list_is_read_as_rows_with_their_arguments() {
        let path = write_stack(
            "{\"services\": [\
             {\"name\": \"auth\", \"program\": \"authserver\", \"args\": [\"-c\", \"authserver/authserver.json\"], \"port\": 6112},\
             {\"name\": \"world\", \"program\": \"worldserver\", \"args\": [\"-c\", \"worldserver/worldserver.json\"], \"port\": 6113}\
             ]}",
        );
        let services = load(&path).expect("the list reads");
        let _ = std::fs::remove_file(&path);
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].name, "auth");
        assert_eq!(services[1].args, vec!["-c", "worldserver/worldserver.json"]);
        assert_eq!(services[1].port, 6113);
    }

    #[test]
    fn a_row_without_a_port_is_refused_with_where_it_is() {
        let path = write_stack("{\"services\": [{\"name\": \"a\", \"program\": \"p\"}]}");
        let error = load(&path).expect_err("refused");
        let _ = std::fs::remove_file(&path);
        assert!(error.contains("port"), "{error}");
    }
}
