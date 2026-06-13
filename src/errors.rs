//! Generic top-level error printing for the binary.
//!
//! Permission/consent failures are diagnosed and rendered by
//! [`crate::remediation`], which produces an actionable remedy (a self-consent
//! hint or a tenant admin-consent URL). This printer is the plain fallback for
//! everything else.

pub fn print(err: &anyhow::Error) {
    eprintln!("Error: {err:#}");
}
