use super::super::browser::storage_get;
use super::super::types::OverviewResponse;

pub(super) fn empty_overview() -> OverviewResponse {
    OverviewResponse {
        stats: Default::default(),
        queues: Vec::new(),
        workers: Vec::new(),
        active_workers: Vec::new(),
    }
}

pub(super) fn stored_theme() -> String {
    storage_get("gw-admin-theme").unwrap_or_else(|| "system".to_string())
}

pub(super) fn stored_accent() -> String {
    storage_get("gw-admin-accent").unwrap_or_else(|| "cyan".to_string())
}

pub(super) fn stored_compact_density() -> bool {
    storage_get("gw-admin-density").as_deref() == Some("compact")
}
