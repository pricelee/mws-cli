use crate::cli::{AuthAction, AuthArgs};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: AuthArgs) -> anyhow::Result<()> {
    match args.action {
        AuthAction::Login(_) => {
            let _ = ctx;
            anyhow::bail!("auth login not implemented yet (Task 10)")
        }
    }
}
