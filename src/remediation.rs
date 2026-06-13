//! Permission/consent failure diagnosis and remediation.
//!
//! When a command fails because a scope is missing or ungranted, this module
//! decides whether the user can self-consent (`auth login --scope X`) or needs a
//! tenant admin, and renders a ready-to-act remediation — inline text for humans,
//! a structured object for agents. It owns the single source of truth for "does
//! this scope require admin consent", plus the shared admin-consent URL renderer
//! used by both this path and the `auth admin-consent` command.

use std::fmt::Write as _;

use crate::auth::DEFAULT_SCOPES;
use crate::context::CliContext;
use crate::graph::GraphError;
use crate::output::Format;

/// Microsoft's hosted "your consent was recorded" page. Public clients like the
/// Microsoft Graph CLI app register this redirect, so the admin's browser lands
/// somewhere meaningful after granting consent.
pub const DEFAULT_ADMIN_REDIRECT: &str =
    "https://login.microsoftonline.com/common/oauth2/nativeclient";

/// Delegated scopes that require admin consent but do NOT end in `.All`, so the
/// suffix heuristic alone would miss them. Keep small and curated.
const CURATED_ADMIN_SCOPES: &[&str] = &["ChannelMessage.ReadWrite"];

// ---------------------------------------------------------------------------
// Scope classification — single source of truth
// ---------------------------------------------------------------------------

/// Does this delegated scope require tenant-admin consent?
///
/// Precedence:
/// 1. Anything in `DEFAULT_SCOPES` is user-consentable by definition — that is the
///    whole point of the default set — even the `.ReadBasic.All` Teams scopes.
/// 2. Otherwise a `*.All` suffix signals an admin scope.
/// 3. A small curated set of non-`.All` admin scopes.
/// 4. Everything else is user-consentable.
///
/// For the runtime-403 case only scope *names* are known, so this is a
/// *prediction*. The login-time path trusts Azure's error code instead and
/// self-corrects when this guess is wrong (e.g. a tenant that disables user
/// consent entirely), which is why the login fallback is load-bearing.
pub fn requires_admin_consent(scope: &str) -> bool {
    if DEFAULT_SCOPES.contains(&scope) {
        return false;
    }
    if scope.ends_with(".All") {
        return true;
    }
    CURATED_ADMIN_SCOPES.contains(&scope)
}

// ---------------------------------------------------------------------------
// Remediation shape
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentKind {
    AdminConsent,
    UserConsent,
}

/// An actionable remedy for a permission failure. Serializes to the agent-facing
/// `remediation` object.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Remediation {
    #[serde(rename = "type")]
    pub kind: ConsentKind,
    pub scopes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consent_url: Option<String>,
    pub next_steps: Vec<String>,
}

/// An error that already carries its remediation and exit code. Commands with the
/// context to build a precise remedy (e.g. `auth login`, which knows the requested
/// scopes) return this; `main` renders it directly without re-analysis.
#[derive(Debug)]
pub struct ConsentError {
    pub message: String,
    pub exit_code: u8,
    pub remediation: Remediation,
}

impl std::fmt::Display for ConsentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConsentError {}

// ---------------------------------------------------------------------------
// Building remediations
// ---------------------------------------------------------------------------

/// Build a remediation for a chosen set of scopes. For `AdminConsent` the
/// consent URL targets the resolved tenant; for `UserConsent` there is no URL —
/// the user simply re-runs sign-in.
pub fn build(ctx: &CliContext, kind: ConsentKind, scopes: Vec<String>) -> Remediation {
    let login_cmd = format!("mws-cli auth login --scope {}", scopes.join(" --scope "));
    let consent_url = match kind {
        ConsentKind::AdminConsent => {
            let tenant = resolve_admin_tenant(ctx);
            Some(build_admin_consent_url(
                &tenant,
                &ctx.client_id,
                &scopes,
                DEFAULT_ADMIN_REDIRECT,
            ))
        }
        ConsentKind::UserConsent => None,
    };
    Remediation {
        kind,
        scopes,
        consent_url,
        next_steps: vec![login_cmd, "<re-run your original command>".to_string()],
    }
}

/// Decide the remediation route for a runtime-403 one-of candidate list. Graph
/// lists candidates narrow→broad; prefer the first self-consentable one (no admin
/// needed), else the narrowest admin scope. None for an empty list.
fn route_runtime(candidates: &[String]) -> Option<(ConsentKind, String)> {
    if candidates.is_empty() {
        return None;
    }
    if let Some(s) = candidates.iter().find(|s| !requires_admin_consent(s)) {
        return Some((ConsentKind::UserConsent, s.clone()));
    }
    Some((ConsentKind::AdminConsent, candidates[0].clone()))
}

/// The admin-requiring subset of a requested scope set (login-time path, where
/// the requested scopes are known precisely). Falls back to the full set when the
/// heuristic flags none — Azure said admin consent is required, so we must offer
/// *something* even if our name-based guess disagrees.
pub fn admin_subset_or_all(requested: &[String]) -> Vec<String> {
    let subset: Vec<String> = requested
        .iter()
        .filter(|s| requires_admin_consent(s))
        .cloned()
        .collect();
    if subset.is_empty() {
        requested.to_vec()
    } else {
        subset
    }
}

// ---------------------------------------------------------------------------
// Diagnosing a top-level error
// ---------------------------------------------------------------------------

/// Inspect a command error and, if it is a permission failure the user can act on,
/// return `(exit_code, optional remediation)`. Returns None for errors that are
/// not permission-related so the generic printer handles them.
///
/// Every "Missing scope permissions" 403 returns `Some` (with `None` remediation
/// when no scopes parse) so the legacy scope hint can never also fire.
pub fn analyze_runtime(ctx: &CliContext, err: &anyhow::Error) -> Option<(u8, Option<Remediation>)> {
    let text = format!("{err:#}");
    if text.contains("Missing scope permissions") {
        let candidates = parse_required_scopes(&text);
        let rem = route_runtime(&candidates).map(|(kind, scope)| build(ctx, kind, vec![scope]));
        return Some((4, rem));
    }
    if is_forbidden(err, &text) {
        return Some((4, None));
    }
    None
}

/// Whether the error is a Graph 403 (permission), via the typed error when
/// available or the formatted string for the `raw` command's plain bail.
fn is_forbidden(err: &anyhow::Error, text: &str) -> bool {
    if let Some(GraphError::Api { status, .. }) = err.downcast_ref::<GraphError>() {
        if *status == 403 {
            return true;
        }
    }
    text.contains("returned 403") || text.contains("graph 403")
}

/// Parse the candidate scope list from a Graph 403 "Missing scope permissions"
/// message: `... API requires one of 'A, B, C'. ...`, in Graph's order.
fn parse_required_scopes(text: &str) -> Vec<String> {
    let Some(list) = extract_between(text, "API requires one of '", "'") else {
        return Vec::new();
    };
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn extract_between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let s = haystack.find(start)? + start.len();
    let rest = &haystack[s..];
    let e = rest.find(end)?;
    Some(&rest[..e])
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render a remediation to stderr: a structured object on JSON (the non-TTY/agent
/// default), human text otherwise. Never opens a browser or sends anything.
pub fn print(ctx: &CliContext, message: &str, remediation: Option<&Remediation>) {
    if ctx.format == Format::Json {
        let body = serde_json::json!({
            "error": { "message": message },
            "remediation": remediation,
        });
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&body).unwrap_or_else(|_| message.to_string())
        );
        return;
    }
    let mut buf = String::new();
    let _ = writeln!(buf, "Error: {message}");
    if let Some(r) = remediation {
        let _ = writeln!(buf);
        render_human(&mut buf, ctx, r);
    }
    eprint!("{buf}");
}

fn render_human(buf: &mut String, ctx: &CliContext, r: &Remediation) {
    let login_cmd = r.next_steps.first().map(String::as_str).unwrap_or("");
    match r.kind {
        ConsentKind::UserConsent => {
            let _ = writeln!(buf, "This needs a permission you can grant yourself:");
            for s in &r.scopes {
                let _ = writeln!(buf, "  - {s}");
            }
            let _ = writeln!(buf);
            let _ = writeln!(buf, "Grant it and retry:");
            let _ = writeln!(buf, "  1. {login_cmd}");
            let _ = writeln!(buf, "  2. Re-run your original command.");
        }
        ConsentKind::AdminConsent => {
            let _ = writeln!(
                buf,
                "This needs a permission your tenant grants only via admin consent:"
            );
            for s in &r.scopes {
                let _ = writeln!(buf, "  - {s}");
            }
            if let Some(url) = &r.consent_url {
                let _ = writeln!(buf);
                let _ = writeln!(buf, "Send this admin-consent URL to a tenant administrator:");
                let _ = writeln!(buf);
                let _ = writeln!(buf, "  {url}");
                if let Some(requester) = current_requester_label(ctx) {
                    let _ = writeln!(buf);
                    let _ = writeln!(buf, "Requesting on behalf of: {requester}");
                }
            }
            let _ = writeln!(buf);
            let _ = writeln!(buf, "After your admin clicks Accept:");
            let _ = writeln!(buf, "  1. Re-run sign-in:  {login_cmd}");
            let _ = writeln!(buf, "  2. Re-run your original command.");
        }
    }
}

// ---------------------------------------------------------------------------
// Shared admin-consent rendering (moved from commands::auth so both the command
// and this remediation path use one source of truth)
// ---------------------------------------------------------------------------

/// Build the admin-consent URL for a tenant + client + scope set. Pure function.
///
/// Uses the v2 endpoint (`/v2.0/adminconsent`), which honors the `scope` query
/// param so the admin consents only to what we list. The v1 endpoint ignores
/// `scope` and falls back to the app's broad static permissions.
pub fn build_admin_consent_url(
    tenant: &str,
    client_id: &str,
    scopes: &[String],
    redirect_uri: &str,
) -> String {
    use url::Url;
    let mut url = Url::parse(&format!(
        "https://login.microsoftonline.com/{tenant}/v2.0/adminconsent"
    ))
    .expect("valid base url");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &scopes.join(" "));
    url.to_string()
}

/// `common` / `organizations` / `consumers` are Microsoft's multi-tenant
/// placeholders — useless as a target for tenant-wide admin consent.
fn is_placeholder_tenant(t: &str) -> bool {
    matches!(t, "common" | "organizations" | "consumers")
}

/// Resolve the tenant to target for admin-consent:
/// 1. Honor `--tenant` if it is concrete (not a placeholder).
/// 2. Otherwise read the signed-in account's stored tenant.
/// 3. If that is still a placeholder but the cached id_token has a `tid`, promote
///    and persist it.
/// 4. Fall back to "organizations" — Microsoft rejects "common"/"consumers" at
///    /v2.0/adminconsent with AADSTS9002328.
pub fn resolve_admin_tenant(ctx: &CliContext) -> String {
    if !is_placeholder_tenant(&ctx.tenant) {
        return ctx.tenant.clone();
    }
    if let Ok(mut account) = ctx.store.load(&ctx.account_name) {
        if !is_placeholder_tenant(&account.tenant) {
            return account.tenant;
        }
        if let Some(it) = account.id_token.as_deref() {
            if let Some(tid) = crate::auth::token::extract_tid(it) {
                account.tenant = tid.clone();
                let _ = ctx.store.save(&account);
                return tid;
            }
        }
    }
    "organizations".to_string()
}

/// Identity of the signed-in user, derived from cached id_token claims. Returns a
/// string like `"Lee Junho <pricelee@contoso.com>"`, or None if nothing useful is
/// cached.
pub fn current_requester_label(ctx: &CliContext) -> Option<String> {
    let account = ctx.store.load(&ctx.account_name).ok()?;
    let id_token = account.id_token.as_deref()?;
    let claims = crate::auth::token::extract_claims(id_token)?;
    let name = claims.get("name").and_then(|v| v.as_str());
    let upn = claims
        .get("preferred_username")
        .or_else(|| claims.get("upn"))
        .or_else(|| claims.get("email"))
        .and_then(|v| v.as_str());
    match (name, upn) {
        (Some(n), Some(u)) => Some(format!("{n} <{u}>")),
        (Some(n), None) => Some(n.to_string()),
        (None, Some(u)) => Some(u.to_string()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scopes_are_user_consentable_even_when_all_suffixed() {
        // The whole point of DEFAULT_SCOPES: user-consentable by definition.
        assert!(!requires_admin_consent("Team.ReadBasic.All"));
        assert!(!requires_admin_consent("Channel.ReadBasic.All"));
        assert!(!requires_admin_consent("User.Read"));
        assert!(!requires_admin_consent("Mail.Send"));
    }

    #[test]
    fn all_suffix_outside_defaults_requires_admin() {
        assert!(requires_admin_consent("Sites.Read.All"));
        assert!(requires_admin_consent("Directory.Read.All"));
        assert!(requires_admin_consent("User.Read.All"));
    }

    #[test]
    fn curated_non_all_admin_scope() {
        assert!(requires_admin_consent("ChannelMessage.ReadWrite"));
    }

    #[test]
    fn ordinary_scope_is_user_consentable() {
        assert!(!requires_admin_consent("Mail.Read"));
        assert!(!requires_admin_consent("Calendars.ReadWrite.Shared"));
    }

    #[test]
    fn parse_required_scopes_from_real_graph_403() {
        let text = r#"graph /me/joinedTeams returned 403 Forbidden: {"error":{"code":"Forbidden","message":"Missing scope permissions on the request. API requires one of 'Team.ReadBasic.All, TeamSettings.Read.All, User.Read.All, Directory.Read.All'. Scopes on the request 'openid, profile, User.Read, email'","innerError":{}}}"#;
        let scopes = parse_required_scopes(text);
        assert_eq!(scopes.first().map(String::as_str), Some("Team.ReadBasic.All"));
        assert!(scopes.contains(&"Directory.Read.All".to_string()));
    }

    #[test]
    fn parse_required_scopes_empty_when_absent() {
        assert!(parse_required_scopes("graph /me returned 401 Unauthorized: {}").is_empty());
    }

    #[test]
    fn route_prefers_self_consentable_candidate() {
        // The canonical /me/joinedTeams list: Team.ReadBasic.All is a default ⇒
        // user-consentable ⇒ route there, no admin needed.
        let candidates = vec![
            "Team.ReadBasic.All".to_string(),
            "Directory.Read.All".to_string(),
        ];
        let (kind, scope) = route_runtime(&candidates).unwrap();
        assert_eq!(kind, ConsentKind::UserConsent);
        assert_eq!(scope, "Team.ReadBasic.All");
    }

    #[test]
    fn route_falls_back_to_narrowest_admin_when_all_admin() {
        let candidates = vec![
            "Sites.Read.All".to_string(),
            "Directory.ReadWrite.All".to_string(),
        ];
        let (kind, scope) = route_runtime(&candidates).unwrap();
        assert_eq!(kind, ConsentKind::AdminConsent);
        assert_eq!(scope, "Sites.Read.All");
    }

    #[test]
    fn route_none_for_empty() {
        assert!(route_runtime(&[]).is_none());
    }

    #[test]
    fn admin_subset_filters_to_admin_scopes() {
        let requested = vec![
            "User.Read".to_string(),
            "Sites.Read.All".to_string(),
            "Mail.Send".to_string(),
        ];
        assert_eq!(admin_subset_or_all(&requested), vec!["Sites.Read.All".to_string()]);
    }

    #[test]
    fn admin_subset_falls_back_to_full_when_none_flagged() {
        // Azure said admin consent is required but our name heuristic flags none —
        // offer the whole requested set rather than nothing.
        let requested = vec!["User.Read".to_string(), "Mail.Send".to_string()];
        assert_eq!(admin_subset_or_all(&requested), requested);
    }

    #[test]
    fn admin_consent_url_has_required_params() {
        let url = build_admin_consent_url(
            "contoso.onmicrosoft.com",
            "14d82eec-204b-4c2f-b7e8-296a70dab67e",
            &["User.Read".into(), "Sites.Read.All".into()],
            DEFAULT_ADMIN_REDIRECT,
        );
        assert!(url.starts_with(
            "https://login.microsoftonline.com/contoso.onmicrosoft.com/v2.0/adminconsent?"
        ));
        assert!(url.contains("client_id=14d82eec-204b-4c2f-b7e8-296a70dab67e"));
        assert!(
            url.contains("scope=User.Read+Sites.Read.All")
                || url.contains("scope=User.Read%20Sites.Read.All")
        );
        assert!(url.contains("redirect_uri="));
    }

    #[test]
    fn admin_consent_url_uses_whatever_tenant_we_pass() {
        let url = build_admin_consent_url("organizations", "X", &["openid".into()], DEFAULT_ADMIN_REDIRECT);
        assert!(url.starts_with("https://login.microsoftonline.com/organizations/v2.0/adminconsent?"));
    }

    #[test]
    fn is_placeholder_recognizes_microsoft_aliases() {
        assert!(is_placeholder_tenant("common"));
        assert!(is_placeholder_tenant("organizations"));
        assert!(is_placeholder_tenant("consumers"));
        assert!(!is_placeholder_tenant("contoso.onmicrosoft.com"));
        assert!(!is_placeholder_tenant("a1b2c3d4-e5f6-7890-abcd-ef1234567890"));
    }

    #[test]
    fn remediation_json_shape_admin() {
        let r = Remediation {
            kind: ConsentKind::AdminConsent,
            scopes: vec!["Sites.Read.All".to_string()],
            consent_url: Some("https://example/adminconsent?x=1".to_string()),
            next_steps: vec![
                "mws-cli auth login --scope Sites.Read.All".to_string(),
                "<re-run your original command>".to_string(),
            ],
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["type"], "admin_consent");
        assert_eq!(v["scopes"][0], "Sites.Read.All");
        assert_eq!(v["consent_url"], "https://example/adminconsent?x=1");
        assert_eq!(v["next_steps"][0], "mws-cli auth login --scope Sites.Read.All");
    }

    #[test]
    fn remediation_json_shape_user_omits_url() {
        let r = Remediation {
            kind: ConsentKind::UserConsent,
            scopes: vec!["Chat.Read".to_string()],
            consent_url: None,
            next_steps: vec![
                "mws-cli auth login --scope Chat.Read".to_string(),
                "<re-run your original command>".to_string(),
            ],
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["type"], "user_consent");
        assert!(v.get("consent_url").is_none(), "consent_url should be omitted for user_consent");
    }
}
