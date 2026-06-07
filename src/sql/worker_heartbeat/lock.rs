use graphile_worker_database::{DbExecutorArg, DbParams, DbValue};

use crate::errors::Result;

/// Advisory lock namespace for coordinating stale worker sweeps.
const SWEEP_LOCK_CLASS_ID: i32 = 0x4757_5253; // "GWRS"
const SWEEP_LOCK_OBJECT_ID: i32 = 0x5357_4550; // "SWEP"

pub async fn try_acquire_sweep_lock(mut executor: impl DbExecutorArg) -> Result<bool> {
    let row = executor
        .fetch_optional(
            "SELECT pg_try_advisory_xact_lock($1::integer, $2::integer) AS acquired",
            DbParams::from(vec![
                DbValue::I32(SWEEP_LOCK_CLASS_ID),
                DbValue::I32(SWEEP_LOCK_OBJECT_ID),
            ]),
        )
        .await?;

    Ok(row
        .map(|row| row.try_get::<bool>("acquired"))
        .transpose()?
        .unwrap_or(false))
}
