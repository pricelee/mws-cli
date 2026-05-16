pub mod datetime;
pub mod events;

use crate::cli::{CalendarArgs, CalendarCmd};
use crate::context::CliContext;

pub async fn run(ctx: &CliContext, args: CalendarArgs) -> anyhow::Result<()> {
    match args.cmd {
        CalendarCmd::Events(a) => events::run(ctx, a).await,
        CalendarCmd::Create(_) => anyhow::bail!("calendar create: implemented in Task 2"),
        CalendarCmd::FindTimes(_) => anyhow::bail!("calendar find-times: implemented in Task 3"),
        CalendarCmd::Rsvp(_) => anyhow::bail!("calendar rsvp: implemented in Task 4"),
        CalendarCmd::Cancel(_) => anyhow::bail!("calendar cancel: implemented in Task 5"),
    }
}
