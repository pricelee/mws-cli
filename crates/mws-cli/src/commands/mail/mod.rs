pub mod send;

use crate::cli::{MailAction, MailArgs};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: MailArgs) -> anyhow::Result<()> {
    match args.action {
        MailAction::Send(s) => send::run(ctx, s).await,
    }
}
