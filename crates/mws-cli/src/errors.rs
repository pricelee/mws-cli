//! Pretty error printing for the top-level binary.
//!
//! In particular, Graph 403 responses include the exact scopes the endpoint
//! requires. We surface them as an actionable `mws auth login --scope ...`
//! hint instead of leaving the user to read the raw Graph error.

use std::fmt::Write;

pub fn print(err: &anyhow::Error) {
    let mut buf = String::new();
    let _ = writeln!(buf, "Error: {err:#}");
    if let Some(hint) = scope_hint(&format!("{err:#}")) {
        let _ = writeln!(buf);
        let _ = writeln!(buf, "{hint}");
    }
    eprint!("{buf}");
}

/// If the error string is a Graph "insufficient scope" 403, extract the
/// required scopes and format a re-login hint.
fn scope_hint(text: &str) -> Option<String> {
    // Graph 403 with missing scopes looks like:
    //   "...Missing scope permissions on the request. API requires one of
    //    'Team.ReadBasic.All, Directory.Read.All, ...'.
    //    Scopes on the request 'openid, profile, User.Read, ...'..."
    if !text.contains("Missing scope permissions") {
        return None;
    }
    let required = extract_between(text, "API requires one of '", "'")?;
    let scopes: Vec<&str> = required
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if scopes.is_empty() {
        return None;
    }
    let primary = scopes[0];
    let mut hint = String::new();
    let _ = writeln!(
        hint,
        "This endpoint requires one of these delegated scopes:"
    );
    for s in &scopes {
        let _ = writeln!(hint, "  - {s}");
    }
    let _ = write!(
        hint,
        "\nGrant one and re-run by signing in again:\n  mws auth login --scope {primary}"
    );
    if scopes.len() > 1 {
        let _ = write!(
            hint,
            "\n\n(Most permissive option above is likely admin-consent — pick the narrowest one your tenant allows.)"
        );
    }
    Some(hint)
}

fn extract_between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let s = haystack.find(start)? + start.len();
    let rest = &haystack[s..];
    let e = rest.find(end)?;
    Some(&rest[..e])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_scopes_from_real_graph_403() {
        let err = r#"graph /me/joinedTeams returned 403 Forbidden: {"error":{"code":"Forbidden","message":"Missing scope permissions on the request. API requires one of 'Team.ReadBasic.All, TeamSettings.Read.All, TeamSettings.ReadWrite.All, User.Read.All, Directory.Read.All, User.ReadWrite.All, Directory.ReadWrite.All'. Scopes on the request 'openid, profile, User.Read, email'","innerError":{}}}"#;
        let hint = scope_hint(err).expect("should produce a hint");
        assert!(hint.contains("Team.ReadBasic.All"));
        assert!(hint.contains("Directory.Read.All"));
        assert!(hint.contains("mws auth login --scope Team.ReadBasic.All"));
    }

    #[test]
    fn returns_none_for_unrelated_errors() {
        assert!(scope_hint("Error: account 'work' not found").is_none());
        assert!(scope_hint("graph /me returned 401 Unauthorized: {}").is_none());
    }

    #[test]
    fn handles_single_scope_requirement() {
        let err = "Missing scope permissions on the request. API requires one of 'Mail.Send'. Scopes on the request 'User.Read'";
        let hint = scope_hint(err).unwrap();
        assert!(hint.contains("Mail.Send"));
        assert!(hint.contains("mws auth login --scope Mail.Send"));
        // Single-scope case shouldn't include the "admin-consent" footer
        assert!(!hint.contains("admin-consent"));
    }
}
