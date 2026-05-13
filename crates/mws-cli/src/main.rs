mod cli;
mod commands;
mod context;
mod errors;
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
            errors::print(&e);
            if e.downcast_ref::<safety::SafetyRefused>().is_some() {
                // exit code 4 == "permission/safety refused" per the M0 spec.
                ExitCode::from(4)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
