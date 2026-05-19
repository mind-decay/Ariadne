//! Tier-08 `exit_criteria` 5 — graceful shutdown on EOF.
//!
//! Spawns the `ariadne-mcp` binary, drives the rmcp `initialize` handshake
//! (so we know the server actually came up), then drops the child's
//! stdin. The select! in [`serve_stdio`] watches `running.waiting()` and
//! must observe EOF, drop the catalog handle, and let `main` return
//! success. We assert:
//!   - the child exits within 5s
//!   - the exit status is success (no panic, no error frame)
//!   - `Child::try_wait` confirms reaping (no zombie)
//!
//! Covers the audit finding F2 (tier-08-report `<findings>`).

mod support;

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_exits_cleanly_on_stdin_eof() {
    let (root, _guard) = support::seed_tiny_project();
    let exe = env!("CARGO_BIN_EXE_ariadne-mcp");

    let mut child = Command::new(exe)
        .arg("serve")
        .arg("--root")
        .arg(&root)
        .env("RUST_LOG", "warn")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn ariadne-mcp");

    let mut stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");
    let mut reader = BufReader::new(stdout).lines();

    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "shutdown-test", "version": "0" }
        }
    });
    let mut payload = init.to_string();
    payload.push('\n');
    stdin
        .write_all(payload.as_bytes())
        .await
        .expect("write initialize");
    stdin.flush().await.expect("flush initialize");

    let line = tokio::time::timeout(Duration::from_secs(5), reader.next_line())
        .await
        .expect("read initialize response in time")
        .expect("read initialize line")
        .expect("server returned a line");
    let parsed: serde_json::Value =
        serde_json::from_str(&line).expect("server emitted valid JSON-RPC");
    assert_eq!(parsed["id"], 1, "initialize response id mismatch");
    assert!(
        parsed.get("error").is_none(),
        "initialize returned error: {parsed}"
    );

    drop(stdin);

    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("child exited within timeout")
        .expect("wait returned status");
    assert!(
        status.success(),
        "ariadne-mcp exited non-zero on EOF: {status:?}"
    );

    let reaped = child.try_wait().expect("try_wait");
    assert!(
        reaped.is_some(),
        "child should be reaped after wait, got pending"
    );
}
