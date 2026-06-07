use chrono::Utc;

use super::super::types::{JobState, ListedJob};

pub(super) fn job_state(job: &ListedJob) -> JobState {
    if job.locked_at.is_some() {
        return JobState::Locked;
    }
    if job.attempts >= job.max_attempts {
        return JobState::Failed;
    }
    if job.run_at > Utc::now() {
        return JobState::Scheduled;
    }
    JobState::Ready
}

pub(super) fn state_label(state: JobState) -> &'static str {
    match state {
        JobState::All => "all",
        JobState::Ready => "ready",
        JobState::Scheduled => "scheduled",
        JobState::Locked => "locked",
        JobState::Failed => "failed",
    }
}

pub(super) fn state_color(state: JobState) -> &'static str {
    match state {
        JobState::Ready => "text-emerald-600 dark:text-emerald-300",
        JobState::Scheduled => "text-amber-600 dark:text-amber-300",
        JobState::Locked => "text-cyan-600 dark:text-cyan-300",
        JobState::Failed => "text-rose-600 dark:text-rose-300",
        JobState::All => "",
    }
}
