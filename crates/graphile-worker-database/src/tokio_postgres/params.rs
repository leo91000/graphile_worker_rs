use ::tokio_postgres::types::ToSql;

use crate::{DbParams, DbValue};

fn boxed_param(value: DbValue) -> Box<dyn ToSql + Sync + Send> {
    match value {
        DbValue::Bool(value) => Box::new(value),
        DbValue::BoolOpt(value) => Box::new(value),
        DbValue::I16(value) => Box::new(value),
        DbValue::I16Opt(value) => Box::new(value),
        DbValue::I32(value) => Box::new(value),
        DbValue::I32Opt(value) => Box::new(value),
        DbValue::I64(value) => Box::new(value),
        DbValue::I64Opt(value) => Box::new(value),
        DbValue::Json(value) => Box::new(value),
        DbValue::JsonOpt(value) => Box::new(value),
        DbValue::Text(value) => Box::new(value),
        DbValue::TextOpt(value) => Box::new(value),
        DbValue::TextArray(value) => Box::new(value),
        DbValue::TextArrayOpt(value) => Box::new(value),
        DbValue::I32Array(value) => Box::new(value),
        DbValue::I64Array(value) => Box::new(value),
        DbValue::TimestampTz(value) => Box::new(value),
        DbValue::TimestampTzOpt(value) => Box::new(value),
    }
}

pub(super) fn boxed_params(params: DbParams) -> Vec<Box<dyn ToSql + Sync + Send>> {
    params.values().iter().cloned().map(boxed_param).collect()
}

pub(super) fn param_refs(params: &[Box<dyn ToSql + Sync + Send>]) -> Vec<&(dyn ToSql + Sync)> {
    params
        .iter()
        .map(|param| param.as_ref() as &(dyn ToSql + Sync))
        .collect()
}
