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

pub fn get_update_queue_clause(escaped_schema: &str, param_ord: u8) -> String {
    format!(
        r#",
            q as (
                update {escaped_schema}._private_job_queues as job_queues
                    set
                        locked_by = ${param_ord},
                        locked_at = now()
                from j
                where job_queues.id = j.job_queue_id
            )
        "#
    )
}
