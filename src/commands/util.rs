//! Helpers shared across multiple sugar commands.

use std::io::Read;

/// Resolve a command-line "body-like" argument:
///   `-`            → read all of stdin
///   `@path`        → read the file at `path` as UTF-8
///   anything else  → use as a literal string
pub fn read_body_arg(s: &str) -> anyhow::Result<String> {
    if s == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        return Ok(buf);
    }
    if let Some(path) = s.strip_prefix('@') {
        return Ok(std::fs::read_to_string(path)?);
    }
    Ok(s.to_string())
}

/// Reject obviously bad Graph ids early — empty, whitespace, or anything
/// containing `/`, `\`, `?`, `#`, or a control character. Graph would also
/// reject these, but we want a usage-style error rather than a 400.
pub fn validate_id(kind: &str, id: &str) -> anyhow::Result<()> {
    if id.trim().is_empty() {
        anyhow::bail!("--{kind} must not be empty");
    }
    if id.chars().any(|c| c == '/' || c == '\\' || c == '?' || c == '#' || c.is_control()) {
        anyhow::bail!("--{kind} contains an invalid character: {id:?}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_passthrough() {
        assert_eq!(read_body_arg("hello").unwrap(), "hello");
    }

    #[test]
    fn at_prefix_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("body.txt");
        std::fs::write(&p, "file contents").unwrap();
        let arg = format!("@{}", p.to_string_lossy());
        assert_eq!(read_body_arg(&arg).unwrap(), "file contents");
    }

    #[test]
    fn validate_id_accepts_normal_ids() {
        assert!(validate_id("team", "abc-123_XYZ").is_ok());
        assert!(validate_id("chat", "19:abcdef@thread.tacv2").is_ok());
    }

    #[test]
    fn validate_id_rejects_separators_and_control() {
        for bad in &["", "  ", "a/b", "a\\b", "a?b", "a#b", "a\nb", "a\0b"] {
            assert!(validate_id("x", bad).is_err(), "should reject {bad:?}");
        }
    }
}
