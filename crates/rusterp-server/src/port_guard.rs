//! Ensure listen ports are free: restart prior self (pidfile), then clobber if configured.

use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::config::{PortConflictConfig, PortConflictPolicy};

/// Resolve pidfile path relative to `RUSTERP_HOME` or cwd.
pub fn resolve_pid_file(path: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    if let Ok(home) = std::env::var("RUSTERP_HOME") {
        return Path::new(&home).join(path);
    }
    std::env::current_dir()
        .unwrap_or_else(|_| Path::new(".").to_path_buf())
        .join(path)
}

/// Before binding, clear conflicting listeners per policy.
pub fn ensure_ports_available(
    addrs: &[SocketAddr],
    cfg: &PortConflictConfig,
) -> Result<(), String> {
    for &addr in addrs {
        ensure_port_available(addr, cfg)?;
    }
    Ok(())
}

fn ensure_port_available(addr: SocketAddr, cfg: &PortConflictConfig) -> Result<(), String> {
    if !port_in_use(addr)? {
        return Ok(());
    }

    tracing::warn!(%addr, "listen port already in use");

    let pid_path = resolve_pid_file(&cfg.pid_file);
    if let Some(pid) = read_pid_file(&pid_path) {
        if is_likely_rusterp_server(pid) {
            tracing::info!(pid, "attempting graceful restart of existing rusterp-server");
            signal_graceful_shutdown(pid);
            if wait_until_free(addr, Duration::from_secs(cfg.graceful_secs)) {
                return Ok(());
            }
            if process_alive(pid) {
                tracing::warn!(pid, "prior instance still alive; sending SIGKILL");
                signal_kill(pid);
                if wait_until_free(addr, Duration::from_secs(2)) {
                    return Ok(());
                }
            }
        } else {
            tracing::warn!(pid, "pidfile present but process is not rusterp-server");
        }
    }

    if !port_in_use(addr)? {
        return Ok(());
    }

    match cfg.policy {
        PortConflictPolicy::Fail => {
            return Err(format!(
                "port {addr} still in use after restart attempt (policy=fail)"
            ));
        }
        PortConflictPolicy::Clobber => {
            tracing::warn!(%addr, "clobbering port occupant (policy=clobber)");
            clobber_port(addr)?;
            if port_in_use(addr)? {
                return Err(format!("port {addr} still in use after clobber"));
            }
        }
    }

    Ok(())
}

pub fn write_pid_file(path: &Path, pid: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create pid dir {}: {e}", parent.display()))?;
    }
    std::fs::write(path, format!("{pid}\n"))
        .map_err(|e| format!("write pid file {}: {e}", path.display()))
}

pub fn remove_pid_file(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn read_pid_file(path: &Path) -> Option<u32> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse().ok()
}

fn port_in_use(addr: SocketAddr) -> Result<bool, String> {
    match std::net::TcpListener::bind(addr) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => Ok(true),
        Err(e) => Err(format!("bind probe on {addr}: {e}")),
    }
}

fn wait_until_free(addr: SocketAddr, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if !port_in_use(addr).unwrap_or(true) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[cfg(unix)]
fn signal_graceful_shutdown(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
}

#[cfg(unix)]
fn signal_kill(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
}

#[cfg(not(unix))]
fn signal_graceful_shutdown(_pid: u32) {}

#[cfg(not(unix))]
fn signal_kill(_pid: u32) {}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn is_likely_rusterp_server(pid: u32) -> bool {
    let cmdline = format!("/proc/{pid}/cmdline");
    let Ok(raw) = std::fs::read(&cmdline) else {
        return false;
    };
    let text = raw
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    text.contains("rusterp-server")
}

#[cfg(not(unix))]
fn is_likely_rusterp_server(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn clobber_port(addr: SocketAddr) -> Result<(), String> {
    use std::process::Command;

    let port = addr.port();
    let output = Command::new("fuser")
        .args(["-k", &format!("{port}/tcp")])
        .output()
        .map_err(|e| format!("fuser failed (install psmisc?): {e}"))?;

    if !output.status.success() && output.status.code() != Some(1) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("fuser -k {port}/tcp failed: {stderr}"));
    }

    std::thread::sleep(Duration::from_millis(200));
    Ok(())
}

#[cfg(not(unix))]
fn clobber_port(addr: SocketAddr) -> Result<(), String> {
    Err(format!("port clobber unsupported on this OS ({addr})"))
}
