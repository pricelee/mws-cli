pub mod create;
pub mod datetime;
pub mod events;
pub mod find_times;

use crate::cli::{CalendarArgs, CalendarCmd};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: CalendarArgs) -> anyhow::Result<()> {
    match args.cmd {
        CalendarCmd::Events(a) => events::run(ctx, a).await,
        CalendarCmd::Create(a) => create::run(ctx, a).await,
        CalendarCmd::FindTimes(a) => find_times::run(ctx, a).await,
        CalendarCmd::Rsvp(_) => anyhow::bail!("calendar rsvp: implemented in Task 4"),
        CalendarCmd::Cancel(_) => anyhow::bail!("calendar cancel: implemented in Task 5"),
    }
}
