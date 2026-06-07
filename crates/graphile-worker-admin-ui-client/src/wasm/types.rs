pub(crate) use graphile_worker_admin_api::jobs::{
    AddJobRequest, JobAction, JobActionRequest, JobActionResponse, JobKeyModeRequest, JobState,
    ListJobsResponse, ListedJob, RemoveJobByKeyRequest,
};
pub(crate) use graphile_worker_admin_api::maintenance::{
    CleanupTaskName, MaintenanceAction, MaintenanceRequest,
};
pub(crate) use graphile_worker_admin_api::overview::{JobStats, OverviewResponse};
pub(crate) use graphile_worker_admin_api::responses::{ErrorResponse, MessageResponse};

#[derive(Clone, Debug)]
pub(crate) struct AdminClientConfig {
    pub(crate) auth_mode: AuthMode,
    pub(crate) auth_header: String,
    pub(crate) csrf: String,
    pub(crate) csrf_header: String,
    pub(crate) read_only: bool,
    pub(crate) schema: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthMode {
    Basic,
    Bearer,
    Header,
    None,
}

impl AuthMode {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "basic" => Ok(Self::Basic),
            "bearer" => Ok(Self::Bearer),
            "header" => Ok(Self::Header),
            "none" => Ok(Self::None),
            value => Err(format!("unsupported auth mode `{value}`")),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Bearer => "bearer",
            Self::Header => "header",
            Self::None => "none",
        }
    }

    pub(crate) fn requires_token(self) -> bool {
        matches!(self, Self::Bearer | Self::Header)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Modal {
    AddJob,
    FailJobs,
    Reschedule,
    RemoveKey,
    JobDetails(ListedJob),
}
