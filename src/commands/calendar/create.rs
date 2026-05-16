//! `mws-cli calendar create` — POST /me/events.

use crate::auth::Endpoints;
use crate::cli::CreateArgs;
use crate::commands::util::read_body_arg;
use crate::context::CliContext;
use crate::graph::GraphClient;
use crate::output::write;
use serde_json::{json, Value};

use super::datetime::{graph_dt, parse_rfc3339_utc};

pub async fn run(ctx: &CliContext, args: CreateArgs) -> anyhow::Result<()> {
    let body = build_event_body(&args)?;
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
            "/me/events",
            Some(bytes),
            &[("Content-Type".into(), "application/json".into())],
        )
        .await?;
    ctx.store.save(&account)?;
    if !status.is_success() {
        anyhow::bail!(
            "POST /me/events returned {}: {}",
            status,
            String::from_utf8_lossy(&resp_body)
        );
    }
    let value: Value = serde_json::from_slice(&resp_body)?;
    let mut stdout = std::io::stdout().lock();
    write(ctx.format, &value, &mut stdout)?;
    Ok(())
}

pub fn build_event_body(args: &CreateArgs) -> anyhow::Result<Value> {
    let start = graph_dt(parse_rfc3339_utc(&args.start)?);
    let end = graph_dt(parse_rfc3339_utc(&args.end)?);
    let tz = args.timezone.clone().unwrap_or_else(|| "UTC".to_string());

    let body_text = match args.body.as_deref() {
        Some(s) => Some(read_body_arg(s)?),
        None => None,
    };
    let body_obj = body_text.map(|content| {
        json!({
            "contentType": if args.html { "html" } else { "text" },
            "content": content,
        })
    });

    let attendees: Vec<Value> = args
        .attendees
        .iter()
        .map(|email| {
            json!({
                "emailAddress": {"address": email},
                "type": "required",
            })
        })
        .collect();

    let mut event = json!({
        "subject": args.subject,
        "start": {"dateTime": start, "timeZone": tz},
        "end":   {"dateTime": end,   "timeZone": tz},
        "attendees": attendees,
    });
    if let Some(b) = body_obj {
        event["body"] = b;
    }
    if let Some(loc) = &args.location {
        event["location"] = json!({"displayName": loc});
    }
    if args.online {
        event["isOnlineMeeting"] = json!(true);
        event["onlineMeetingProvider"] = json!("teamsForBusiness");
    }
    Ok(event)
}

fn dry_run_print(ctx: &CliContext, body: &Value) -> anyhow::Result<()> {
    let preview = json!({
        "dry_run": true,
        "method": "POST",
        "url": format!("{}/me/events", ctx.graph_base),
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
    use crate::cli::CreateArgs;

    fn args() -> CreateArgs {
        CreateArgs {
            subject: "Sync".into(),
            start: "2026-05-17T14:00:00Z".into(),
            end: "2026-05-17T15:00:00Z".into(),
            attendees: vec!["alice@x.com".into()],
            body: None,
            html: false,
            location: None,
            online: false,
            timezone: None,
        }
    }

    #[test]
    fn body_has_subject_and_times() {
        let v = build_event_body(&args()).unwrap();
        assert_eq!(v["subject"], "Sync");
        assert_eq!(v["start"]["dateTime"], "2026-05-17T14:00:00");
        assert_eq!(v["start"]["timeZone"], "UTC");
    }

    #[test]
    fn online_flag_sets_provider() {
        let mut a = args();
        a.online = true;
        let v = build_event_body(&a).unwrap();
        assert_eq!(v["isOnlineMeeting"], true);
        assert_eq!(v["onlineMeetingProvider"], "teamsForBusiness");
    }

    #[test]
    fn attendees_default_to_required() {
        let v = build_event_body(&args()).unwrap();
        assert_eq!(v["attendees"][0]["type"], "required");
        assert_eq!(v["attendees"][0]["emailAddress"]["address"], "alice@x.com");
    }
}
