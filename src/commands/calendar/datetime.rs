//! Shared chrono helpers for the calendar sugar commands.

use chrono::{DateTime, Duration, Utc};

/// Parse an RFC 3339 / ISO 8601 string (with Z or explicit offset) and return UTC.
pub fn parse_rfc3339_utc(s: &str) -> anyhow::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| anyhow::anyhow!("invalid ISO 8601 datetime {s:?}: {e}"))
}

/// Format a UTC instant as Graph's expected dateTime field — no offset, no Z.
pub fn graph_dt(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Default window for `calendar events`: now → now + 7 days.
pub fn default_window() -> (DateTime<Utc>, DateTime<Utc>) {
    let start = Utc::now();
    (start, start + Duration::days(7))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utc_z() {
        let dt = parse_rfc3339_utc("2026-05-16T14:00:00Z").unwrap();
        assert_eq!(graph_dt(dt), "2026-05-16T14:00:00");
    }

    #[test]
    fn parses_offset_and_normalizes_to_utc() {
        let dt = parse_rfc3339_utc("2026-05-16T14:00:00-08:00").unwrap();
        // -08:00 means UTC is 8 hours ahead → 22:00 UTC.
        assert_eq!(graph_dt(dt), "2026-05-16T22:00:00");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_rfc3339_utc("nope").is_err());
        assert!(parse_rfc3339_utc("2026-13-99").is_err());
    }

    #[test]
    fn default_window_is_7_days() {
        let (s, e) = default_window();
        let d = e - s;
        assert_eq!(d.num_days(), 7);
    }
}
