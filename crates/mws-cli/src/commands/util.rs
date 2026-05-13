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
}
