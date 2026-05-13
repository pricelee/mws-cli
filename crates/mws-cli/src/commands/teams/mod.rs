pub mod list;

use crate::cli::{TeamsArgs, TeamsCmd};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: TeamsArgs) -> anyhow::Result<()> {
    match args.cmd {
        TeamsCmd::List => list::run_list(ctx).await,
        TeamsCmd::Channels(c) => list::run_channels(ctx, &c.team).await,
        TeamsCmd::Post(_) => anyhow::bail!("teams post: implemented in Task 3"),
        TeamsCmd::Chats => anyhow::bail!("teams chats: implemented in Task 4"),
        TeamsCmd::Chat(_) => anyhow::bail!("teams chat post: implemented in Task 4"),
        TeamsCmd::Presence => anyhow::bail!("teams presence: implemented in Task 5"),
    }
}
