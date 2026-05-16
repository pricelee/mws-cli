pub mod cp;

use crate::cli::{DriveAction, DriveArgs};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: DriveArgs) -> anyhow::Result<()> {
    match args.action {
        DriveAction::Cp(c) => cp::run(ctx, c).await,
    }
}
