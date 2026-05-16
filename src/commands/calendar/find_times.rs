//! `mws-cli calendar find-times` — POST /me/findMeetingTimes.

use crate::auth::Endpoints;
use crate::cli::FindTimesArgs;
use crate::context::CliContext;
use crate::graph::GraphClient;
use crate::output::write;
use serde_json::{json, Value};

use super::datetime::{default_window, graph_dt, parse_rfc3339_utc};

pub async fn run(ctx: &CliContext, args: FindTimesArgs) -> anyhow::Result<()> {
    let body = build_find_times_body(&args)?;
    if ctx.dry_run {
        return dry_run_print(ctx, &body);
    }
    let mut account = ctx.store.load(&ctx.account_name)?;
    let endpoints = Endpoints::for_tenant(&account.tenant);
    let client = GraphClient::new(ctx.graph_base.clone(), endpoints);
    let bytes = serde_json::to_vec(&body)?;
    let (status, resp_body, _ct) = client
        .send_request(
            &mut account,
            reqwest::Method::POST,
            "/me/findMeetingTimes",
            Some(bytes),
            &[("Content-Type".into(), "application/json".into())],
        )
        .await?;
    ctx.store.save(&account)?;
    if !status.is_success() {
        anyhow::bail!(
            "POST /me/findMeetingTimes returned {}: {}",
            status,
            String::from_utf8_lossy(&resp_body)
        );
    }
    let value: Value = serde_json::from_slice(&resp_body)?;
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &value, &mut stdout)?;
    Ok(())
}

pub fn build_find_times_body(args: &FindTimesArgs) -> anyhow::Result<Value> {
    let (s, e) = match (args.start.as_deref(), args.end.as_deref()) {
        (Some(s), Some(e)) => (parse_rfc3339_utc(s)?, parse_rfc3339_utc(e)?),
        (Some(s), None) => {
            let s = parse_rfc3339_utc(s)?;
            (s, s + chrono::Duration::days(7))
        }
        (None, _) => default_window(),
    };
    let attendees: Vec<Value> = args
        .attendees
        .iter()
        .map(|email| {
            json!({
                "type": "required",
                "emailAddress": {"address": email},
            })
        })
        .collect();
    let mut body = json!({
        "attendees": attendees,
        "meetingDuration": args.duration,
        "timeConstraint": {
            "timeSlots": [{
                "start": {"dateTime": graph_dt(s), "timeZone": "UTC"},
                "end":   {"dateTime": graph_dt(e), "timeZone": "UTC"},
            }],
        },
    });
    if let Some(top) = args.top {
        body["maxCandidates"] = json!(top);
    }
    Ok(body)
}

fn dry_run_print(ctx: &CliContext, body: &Value) -> anyhow::Result<()> {
    let preview = json!({
        "dry_run": true,
        "method": "POST",
        "url": format!("{}/me/findMeetingTimes", ctx.graph_base),
        "headers": {"Content-Type": "application/json"},
        "body": body,
    });
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &preview, &mut stdout)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::FindTimesArgs;

    fn args() -> FindTimesArgs {
        FindTimesArgs {
            attendees: vec!["alice@x.com".into()],
            duration: "PT30M".into(),
            start: Some("2026-05-17T00:00:00Z".into()),
            end: Some("2026-05-18T00:00:00Z".into()),
            top: Some(5),
        }
    }

    #[test]
    fn body_has_duration_and_attendees() {
        let v = build_find_times_body(&args()).unwrap();
        assert_eq!(v["meetingDuration"], "PT30M");
        assert_eq!(v["attendees"][0]["emailAddress"]["address"], "alice@x.com");
    }

    #[test]
    fn body_has_time_slot() {
        let v = build_find_times_body(&args()).unwrap();
        let slot = &v["timeConstraint"]["timeSlots"][0];
        assert_eq!(slot["start"]["dateTime"], "2026-05-17T00:00:00");
        assert_eq!(slot["end"]["timeZone"], "UTC");
    }

    #[test]
    fn top_becomes_max_candidates() {
        let v = build_find_times_body(&args()).unwrap();
        assert_eq!(v["maxCandidates"], 5);
    }
}
