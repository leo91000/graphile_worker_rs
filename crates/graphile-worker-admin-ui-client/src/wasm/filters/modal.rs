use super::super::types::Modal;

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
