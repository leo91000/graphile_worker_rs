pub fn get_flag_clause(flags_to_skip: &[String], param_ord: u8) -> String {
    if !flags_to_skip.is_empty() {
        return format!("and ((flags ?| ${param_ord}::text[]) is not true)");
    }
    String::new()
}

pub fn get_queue_clause(escaped_schema: &str) -> String {
    format!(
        r#"
            and (
                jobs.job_queue_id is null
                or
                jobs.job_queue_id in (
                    select id
                    from {escaped_schema}._private_job_queues as job_queues
                    where job_queues.is_available = true
                    for update
                    skip locked
                )
            )
        "#
    )
}

pub fn get_update_queue_clause(
    escaped_schema: &str,
    worker_id_param: u8,
    now_param: Option<u8>,
) -> String {
    let locked_at = get_now_clause(now_param);
    format!(
        r#",
            q as (
                update {escaped_schema}._private_job_queues as job_queues
                    set
                        locked_by = ${worker_id_param},
                        locked_at = {locked_at}
                from j
                where job_queues.id = j.job_queue_id
            )
        "#
    )
}

pub fn get_now_clause(now_param: Option<u8>) -> String {
    now_param
        .map(|p| format!("${p}::timestamptz"))
        .unwrap_or_else(|| "now()".to_string())
}
