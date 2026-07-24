//! Performer trait for consuming scheduled intents.

use crate::application::scheduler::events::scheduled_intent::ScheduledIntent;

/// Consumer of scheduled intents.
pub trait Performer {
    /// Performs one scheduled event.
    fn perform(&self, event: ScheduledIntent);
}
