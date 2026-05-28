//! Interface for tmux commands

use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;
use std::process::Stdio;

/// Special target identifier for the previous (last active) pane
const PREVIOUS_PANE_TARGET: &str = "previous";

/// Run a tmux command and return its stdout as a String
fn run_tmux(args: &[&str]) -> Result<String> {
    let output = Command::new("tmux").args(args).output().context(format!(
        "Failed to execute tmux {}",
        args.first().unwrap_or(&"")
    ))?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).context("tmux output contained invalid UTF-8")
}

/// Run a tmux command while piping UTF-8 input to stdin.
fn run_tmux_with_input(args: &[&str], input: &str) -> Result<()> {
    let mut child = Command::new("tmux")
        .args(args)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(format!(
            "Failed to execute tmux {}",
            args.first().unwrap_or(&"")
        ))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .context("Failed to write tmux buffer contents")?;
    }

    let output = child
        .wait_with_output()
        .context(format!("Failed to wait for tmux {}", args.first().unwrap_or(&"")))?;

    if !output.status.success() {
        anyhow::bail!(
            "tmux {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Get the pane ID of the previous (last active) pane in the current window.
///
/// Uses tmux's `pane_last` format variable to find the pane that was active
/// before the current one.
fn get_previous_pane_id() -> Result<String> {
    let pane_id = run_tmux(&["list-panes", "-f", "#{pane_last}", "-F", "#{pane_id}"])?
        .trim()
        .to_string();

    if pane_id.is_empty() {
        anyhow::bail!(
            "No previous pane found. Make sure you have multiple panes in the current window."
        );
    }

    Ok(pane_id)
}

/// Resolve the target string (e.g., "previous", "%1") to a concrete pane ID
///
/// Returns the resolved pane ID that can be used with tmux commands.
/// - `Some("previous")`: resolves to the previous (last active) pane
/// - `Some(id)`: returns the id as-is
/// - `None`: returns the current pane's ID
pub fn resolve_pane_id(target: Option<&str>) -> Result<String> {
    match target {
        Some(PREVIOUS_PANE_TARGET) => get_previous_pane_id(),
        Some(id) => Ok(id.to_string()),
        None => Ok(run_tmux(&["display-message", "-p", "#{pane_id}"])?
            .trim()
            .to_string()),
    }
}

/// Capture the content of a tmux pane
///
/// Uses `tmux capture-pane` with:
/// - `-e`: preserve escape sequences (ANSI colors)
/// - `-J`: join wrapped lines
/// - `-p`: output to stdout
/// - `-S -`: start from the beginning of scrollback history
/// - `-E -`: end at the last line (ensures we capture everything including content below cursor)
///
/// The `pane_id` should be a resolved pane ID (e.g., "%0") from `resolve_pane_id`.
pub fn capture_pane(pane_id: &str) -> Result<String> {
    run_tmux(&[
        "capture-pane",
        "-e",
        "-J",
        "-p",
        "-S",
        "-",
        "-E",
        "-",
        "-t",
        pane_id,
    ])
}

/// Send content to a target pane as literal keys
///
/// Uses `tmux send-keys` with `-l` flag to send text literally without interpreting
/// special characters as key names.
pub fn send_keys(pane_id: &str, content: &str) -> Result<()> {
    run_tmux(&["send-keys", "-t", pane_id, "-l", content])?;
    Ok(())
}

/// List all pane IDs in the current tmux window
///
/// Returns a vector of pane IDs (e.g., ["%0", "%1", "%2"])
pub fn list_panes() -> Result<Vec<String>> {
    let out = run_tmux(&["list-panes", "-F", "#{pane_id}"])?;
    Ok(out
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect())
}

/// Copy content into the tmux paste buffer.
pub fn copy_to_tmux_buffer(content: &str) -> Result<()> {
    run_tmux_with_input(&["load-buffer", "-"], content)
        .context("Failed to copy to tmux buffer")
}
