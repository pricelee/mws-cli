mod auth;
mod cli;
mod commands;
mod context;
mod errors;
mod graph;
mod keyring;
mod output;
mod remediation;
mod safety;

use clap::Parser;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args = cli::Cli::parse();
    let ctx = match context::CliContext::build(&args) {
        Ok(c) => c,
        Err(e) => {
            errors::print(&e);
            return ExitCode::FAILURE;
        }
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| if ctx.verbose { "info".into() } else { "warn".into() }),
        )
        .init();
    let result = match args.command {
        cli::Command::Auth(a) => commands::auth::run(&ctx, a).await,
        cli::Command::Calendar(a) => commands::calendar::run(&ctx, a).await,
        cli::Command::Drive(a) => commands::drive::run(&ctx, a).await,
        cli::Command::Mail(a) => commands::mail::run(&ctx, a).await,
        cli::Command::Raw(a) => commands::raw::run(&ctx, a).await,
        cli::Command::Teams(a) => commands::teams::run(&ctx, a).await,
        cli::Command::Whoami => commands::whoami::run(&ctx).await,
        cli::Command::Describe(a) => commands::describe::run(a),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Safety refusals keep their exact message and exit code 4.
            if e.downcast_ref::<safety::SafetyRefused>().is_some() {
                errors::print(&e);
                return ExitCode::from(4);
            }
            // A command that built its own remediation (e.g. an `auth login`
            // consent failure, which knows the requested scopes).
            if let Some(ce) = e.downcast_ref::<remediation::ConsentError>() {
                remediation::print(&ctx, &ce.message, Some(&ce.remediation));
                return ExitCode::from(ce.exit_code);
            }
            // Runtime permission/consent failures detected from the error itself.
            if let Some((code, rem)) = remediation::analyze_runtime(&ctx, &e) {
                remediation::print(&ctx, &format!("{e:#}"), rem.as_ref());
                return ExitCode::from(code);
            }
            errors::print(&e);
            ExitCode::FAILURE
        }
    }
}
