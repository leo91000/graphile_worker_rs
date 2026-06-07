use leptos::prelude::*;

use super::super::types::ListedJob;
use super::state::{job_state, state_label};

pub(super) fn selected_rows(
    jobs: RwSignal<Vec<ListedJob>>,
    selected: RwSignal<Vec<i64>>,
) -> Vec<ListedJob> {
    let selected = selected.get_untracked();
    jobs.get_untracked()
        .into_iter()
        .filter(|job| selected.contains(&job.id))
        .collect()
}

pub(super) fn selected_csv(rows: &[ListedJob]) -> String {
    let columns = [
        "id",
        "task_identifier",
        "queue_name",
        "state",
        "run_at",
        "attempts",
        "max_attempts",
        "priority",
        "key",
    ];
    let mut output = vec![columns.join(",")];
    for job in rows {
        output.push(
            [
                job.id.to_string(),
                job.task_identifier.clone(),
                job.queue_name.clone().unwrap_or_default(),
                state_label(job_state(job)).to_string(),
                job.run_at.to_rfc3339(),
                job.attempts.to_string(),
                job.max_attempts.to_string(),
                job.priority.to_string(),
                job.key.clone().unwrap_or_default(),
            ]
            .into_iter()
            .map(csv_escape)
            .collect::<Vec<_>>()
            .join(","),
        );
    }
    let mut csv = output.join("\r\n");
    csv.push_str("\r\n");
    csv
}

fn csv_escape(value: String) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
