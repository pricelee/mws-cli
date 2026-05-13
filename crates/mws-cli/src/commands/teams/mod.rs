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
        TeamsCmd::Chats => list::run_chats(ctx).await,
        TeamsCmd::Chat(c) => match c.action {
            crate::cli::ChatAction::Post(p) => {
                post::run_chat_post(ctx, &p.chat, &p.message, p.html).await
            }
        },
        TeamsCmd::Presence => anyhow::bail!("teams presence: implemented in Task 5"),
    }
}
