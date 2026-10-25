use std::process::{Command, Stdio};
use std::time::Duration;
use wowie_observer::native_messaging::{
    read_native_message, write_native_message, BrowserMessage, HostResponse,
};

#[test]
fn test_cli_version_and_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_wowie-browser-host"))
        .arg("--version")
        .output()
        .expect("Failed to run wowie-browser-host --version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("wowie-browser-host 0.1.0"));

    let output_help = Command::new(env!("CARGO_BIN_EXE_wowie-browser-host"))
        .arg("--help")
        .output()
        .expect("Failed to run wowie-browser-host --help");

    assert!(output_help.status.success());
    let stderr = String::from_utf8_lossy(&output_help.stderr);
    assert!(stderr.contains("Native Messaging Host"));
}

#[test]
fn test_browser_host_process_ipc_roundtrip() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_wowie-browser-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn wowie-browser-host");

    let mut stdin = child.stdin.take().expect("Child stdin unavailable");
    let mut stdout = child.stdout.take().expect("Child stdout unavailable");

    // 1. Send valid message
    let msg = BrowserMessage {
        origin: "https://docs.rs/tokio/latest/tokio/".to_string(),
        is_active_window: true,
        tab_id: Some(42),
        title: Some("tokio - Rust".to_string()),
    };

    let payload = serde_json::to_vec(&msg).unwrap();
    write_native_message(&mut stdin, &payload).expect("Failed to write to host stdin");

    let resp_bytes = read_native_message(&mut stdout)
        .expect("Failed to read from host stdout")
        .expect("Unexpected EOF from host");

    let resp: HostResponse = serde_json::from_slice(&resp_bytes).expect("Failed to parse response");
    assert_eq!(resp.status, "ok");
    // Origin must be stripped of path
    assert_eq!(
        resp.acknowledged_origin,
        Some("https://docs.rs".to_string())
    );

    // 2. Send invalid JSON message - host should reply with error, not crash
    let invalid_json = b"{\"invalid_json: 123";
    write_native_message(&mut stdin, invalid_json).expect("Failed to write invalid JSON");

    let resp_err_bytes = read_native_message(&mut stdout)
        .expect("Failed to read error response")
        .expect("Unexpected EOF on error response");

    let resp_err: HostResponse =
        serde_json::from_slice(&resp_err_bytes).expect("Failed to parse err response");
    assert_eq!(resp_err.status, "error");
    assert!(resp_err.error.is_some());

    // 3. Drop stdin (clean EOF), host should exit cleanly
    drop(stdin);

    let status = child
        .wait_timeout(Duration::from_secs(5))
        .expect("Wait error")
        .expect("Host did not exit within timeout");

    assert!(status.success());
}

trait WaitTimeout {
    fn wait_timeout(&mut self, dur: Duration) -> std::io::Result<Option<std::process::ExitStatus>>;
}

impl WaitTimeout for std::process::Child {
    fn wait_timeout(&mut self, dur: Duration) -> std::io::Result<Option<std::process::ExitStatus>> {
        let start = std::time::Instant::now();
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(Some(status));
            }
            if start.elapsed() >= dur {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
