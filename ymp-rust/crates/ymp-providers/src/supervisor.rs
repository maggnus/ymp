//! A short-lived parent for native agent processes. It remains alive while the
//! host's stdin is open; losing the host closes that pipe even after SIGKILL.
use anyhow::{Context, Result};
use std::{ffi::OsString, process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};

pub async fn run(argv: Vec<OsString>) -> Result<()> {
    let program = argv.first().context("Missing supervised command")?;
    let mut child = Command::new(program)
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().context("Missing child stdin")?;
    let stdin = tokio::spawn(async move {
        let result = tokio::io::copy(&mut tokio::io::stdin(), &mut input).await;
        let _ = input.shutdown().await;
        result
    });
    tokio::select! {
        _ = stdin => {},
        _ = child.wait() => {},
    }
    let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
    // The supervisor was started in its own process group. It still owns that
    // group here, so this cannot target a recycled agent PID or the host's group.
    // Kill includes this process and any MCP/command descendants still alive.
    #[cfg(unix)]
    unsafe {
        let group = libc::getpgrp();
        if group == libc::getpid() {
            libc::kill(-group, libc::SIGKILL);
        }
    }
    Ok(())
}
