pub mod cancel;
pub mod create;
pub mod datetime;
pub mod events;
pub mod find_times;
pub mod rsvp;

use crate::cli::{CalendarArgs, CalendarCmd};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: CalendarArgs) -> anyhow::Result<()> {
    match args.cmd {
        CalendarCmd::Events(a) => events::run(ctx, a).await,
        CalendarCmd::Create(a) => create::run(ctx, a).await,
        CalendarCmd::FindTimes(a) => find_times::run(ctx, a).await,
        CalendarCmd::Rsvp(a) => rsvp::run(ctx, a).await,
        CalendarCmd::Cancel(a) => cancel::run(ctx, a).await,
    }
}
