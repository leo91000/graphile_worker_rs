use graphile_worker_job_spec::JobSpec;

#[derive(Debug, Clone)]
pub struct RawJobSpec {
    pub identifier: String,
    pub payload: serde_json::Value,
    pub spec: JobSpec,
}

pub struct JobToAdd<'a> {
    pub identifier: &'a str,
    pub payload: serde_json::Value,
    pub spec: &'a JobSpec,
}
