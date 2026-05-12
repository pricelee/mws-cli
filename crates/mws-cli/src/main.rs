mod cli;
mod commands;
mod context;

use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| if args.verbose { "info".into() } else { "warn".into() }),
        )
        .init();
    let ctx = context::CliContext::build(&args)?;
    match args.command {
        cli::Command::Auth(a) => commands::auth::run(&ctx, a).await?,
        cli::Command::Whoami => commands::whoami::run(&ctx).await?,
    }
    Ok(())
}
