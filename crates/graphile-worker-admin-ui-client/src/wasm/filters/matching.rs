use super::super::types::ListedJob;
use super::format::stringify_value;

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
