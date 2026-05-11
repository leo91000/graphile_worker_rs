use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use leptos::prelude::*;
use serde_json::Value;
use wasm_bindgen::prelude::JsValue;
use wasm_bindgen::JsCast;
use web_sys::{Event, HtmlTextAreaElement};

use super::types::{JobState, ListedJob, Modal};

pub(super) fn filter_values(raw: &str) -> Vec<String> {
    raw.split([',', '\n', '\r'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_lowercase)
        .collect()
}

pub(super) fn normalize_filter_paste(raw: &str) -> String {
    raw.split([',', '\n', '\r', '\t'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn matches_column(job: &ListedJob, column: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let value = match column {
        "task_identifier" => job.task_identifier.clone(),
        "queue_name" => job.queue_name.clone().unwrap_or_default(),
        "key" => job.key.clone().unwrap_or_default(),
        "locked_by" => job.locked_by.clone().unwrap_or_default(),
        _ => String::new(),
    }
    .to_lowercase();
    filters.iter().any(|filter| value.contains(filter))
}

pub(super) fn job_search_text(job: &ListedJob) -> String {
    [
        job.id.to_string(),
        job.task_identifier.clone(),
        job.queue_name.clone().unwrap_or_default(),
        job.key.clone().unwrap_or_default(),
        job.locked_by.clone().unwrap_or_default(),
        job.last_error.clone().unwrap_or_default(),
        stringify_value(&job.payload),
    ]
    .join(" ")
    .to_lowercase()
}

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

pub(super) fn csv_escape(value: String) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(super) fn stringify_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

pub(super) fn short(value: &str, length: usize) -> String {
    if value.chars().count() <= length {
        return value.to_string();
    }
    let mut output = value
        .chars()
        .take(length.saturating_sub(3))
        .collect::<String>();
    output.push_str("...");
    output
}

pub(super) fn format_date(value: DateTime<Utc>) -> String {
    value.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

pub(super) fn textarea_value(event: &Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<HtmlTextAreaElement>().ok())
        .map(|target| target.value())
        .unwrap_or_default()
}

pub(super) fn optional_string(value: String) -> Option<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub(super) fn optional_i16(value: String) -> Option<i16> {
    value.trim().parse().ok()
}

pub(super) fn optional_csv(value: String) -> Option<Vec<String>> {
    let values = value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

pub(super) fn datetime_local_to_utc(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let format = if value.matches(':').count() >= 2 {
        "%Y-%m-%dT%H:%M:%S"
    } else {
        "%Y-%m-%dT%H:%M"
    };
    NaiveDateTime::parse_from_str(value, format)
        .ok()
        .and_then(|datetime| {
            let js_local = js_sys::Date::new(&JsValue::from_str(
                &datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
            ));
            let offset_minutes = js_local.get_timezone_offset();
            if !offset_minutes.is_finite() {
                return None;
            }
            Some(DateTime::<Utc>::from_naive_utc_and_offset(
                datetime + Duration::minutes(offset_minutes as i64),
                Utc,
            ))
        })
}

pub(super) fn modal_title(modal: &Option<Modal>) -> &'static str {
    match modal {
        Some(Modal::AddJob) => "Add job",
        Some(Modal::FailJobs) => "Fail selected jobs",
        Some(Modal::Reschedule) => "Reschedule selected jobs",
        Some(Modal::RemoveKey) => "Remove job by key",
        Some(Modal::JobDetails(_)) => "Job details",
        None => "",
    }
}
