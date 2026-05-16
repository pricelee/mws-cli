//! Destructive-operation guard for `mws-cli raw`.
//!
//! Microsoft Graph endpoints invoked through `mws-cli raw` can be permanently
//! destructive (delete a channel, delete a message, revoke a grant). When the
//! user — or an AI agent driving mws-cli — issues one of these, we want a
//! confirmation step instead of silently dispatching.
//!
//! Policy:
//!   1. `--yes` (or `-y`) bypasses everything. Silent proceed.
//!   2. If stdin is a TTY (interactive user): prompt `[y/N]`. Default no.
//!   3. If stdin is NOT a TTY (pipe / AI agent / cron): refuse with exit 4
//!      and a hint to pass `--yes`. No prompt — we have nowhere to read it
//!      from, and accepting `echo y |` would be a footgun.
//!
//! `mws-cli auth logout --all` is destructive but local-only (no Graph call) so
//! it's intentionally outside this module's scope.

use std::io::{IsTerminal, Write};

/// Returned (via anyhow chain) when a destructive op was refused. main.rs
/// downcasts this to set exit code 4.
#[derive(Debug)]
pub struct SafetyRefused;

impl std::fmt::Display for SafetyRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("destructive operation refused")
    }
}

impl std::error::Error for SafetyRefused {}

/// Decide whether the given `(method, path)` is destructive enough to gate.
pub fn is_destructive(method: &str, path: &str) -> bool {
    let m = method.to_ascii_uppercase();
    if m == "DELETE" {
        return true;
    }
    if m == "POST" {
        // Path part only (strip query string).
        let p = path.split('?').next().unwrap_or(path).trim_end_matches('/');
        for suffix in [
            "/delete",
            "/permanentDelete",
            "/revokeGrants",
            "/archive",
        ] {
            if p.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}

/// Gate the operation. Returns `Ok(())` if the caller may proceed,
/// `Err(SafetyRefused)` otherwise (wrapped in anyhow for the error chain).
pub fn gate(method: &str, path: &str, yes: bool) -> anyhow::Result<()> {
    if !is_destructive(method, path) {
        return Ok(());
    }
    if yes {
        return Ok(());
    }
    if std::io::stdin().is_terminal() {
        prompt(method, path)
    } else {
        let msg = format!(
            "destructive operation refused: {method} {path}\n\
             stdin is not a TTY, so mws-cli cannot prompt for confirmation.\n\
             Re-run with --yes to acknowledge."
        );
        Err(anyhow::Error::new(SafetyRefused).context(msg))
    }
}

fn prompt(method: &str, path: &str) -> anyhow::Result<()> {
    eprintln!("About to execute a destructive Graph operation:");
    eprintln!("  {method} {path}");
    eprint!("Proceed? [y/N] ");
    let _ = std::io::stderr().flush();

    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();
    if answer == "y" || answer == "yes" {
        Ok(())
    } else {
        Err(anyhow::Error::new(SafetyRefused).context("user declined"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_is_destructive() {
        assert!(is_destructive("DELETE", "/me/messages/AAA"));
        assert!(is_destructive("delete", "/teams/X/channels/Y")); // case-insensitive
    }

    #[test]
    fn post_with_destructive_suffix() {
        assert!(is_destructive("POST", "/me/messages/AAA/permanentDelete"));
        assert!(is_destructive("POST", "/me/drive/items/AAA/permanentDelete"));
        assert!(is_destructive("POST", "/drives/X/items/Y/delete"));
        assert!(is_destructive("POST", "/me/drive/items/A/permissions/B/revokeGrants"));
        assert!(is_destructive("POST", "/teams/X/archive"));
    }

    #[test]
    fn destructive_post_with_query_string() {
        assert!(is_destructive("POST", "/teams/X/archive?$select=id"));
    }

    #[test]
    fn safe_operations_pass() {
        assert!(!is_destructive("GET", "/me/messages"));
        assert!(!is_destructive("GET", "/me"));
        assert!(!is_destructive("POST", "/me/sendMail"));
        assert!(!is_destructive("POST", "/me/messages"));
        assert!(!is_destructive("POST", "/chats/X/messages"));
        assert!(!is_destructive("POST", "/teams/X/channels/Y/messages"));
        assert!(!is_destructive("PUT", "/me/drive/root:/foo.txt:/content"));
        assert!(!is_destructive("PATCH", "/me/messages/AAA"));
    }

    #[test]
    fn gate_passes_safe_ops() {
        gate("GET", "/me", false).expect("safe ops always pass");
    }

    #[test]
    fn gate_passes_with_yes() {
        gate("DELETE", "/me/messages/AAA", true).expect("--yes should bypass");
    }
}
