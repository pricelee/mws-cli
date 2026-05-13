pub mod list;
pub mod post;

use crate::cli::{TeamsArgs, TeamsCmd};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: TeamsArgs) -> anyhow::Result<()> {
    match args.cmd {
        TeamsCmd::List => list::run_list(ctx).await,
        TeamsCmd::Channels(c) => list::run_channels(ctx, &c.team).await,
        TeamsCmd::Post(p) => {
            post::run_channel_post(ctx, &p.team, &p.channel, &p.message, p.html).await
        }
        TeamsCmd::Chats => anyhow::bail!("teams chats: implemented in Task 4"),
        TeamsCmd::Chat(_) => anyhow::bail!("teams chat post: implemented in Task 4"),
        TeamsCmd::Presence => anyhow::bail!("teams presence: implemented in Task 5"),
    }
}
