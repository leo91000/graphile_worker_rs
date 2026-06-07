use chrono::{DateTime, Local, Utc};
use serde_json::Value;

use crate::DbError;

use super::DbCell;

pub trait FromDbCell: Sized {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError>;
}

fn type_error(name: &str, expected: &str, cell: &DbCell) -> DbError {
    DbError::new(format!(
        "column `{name}` could not be decoded as {expected}; actual value was {cell:?}"
    ))
}

impl FromDbCell for bool {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::Bool(value) => Ok(*value),
            _ => Err(type_error(name, "bool", cell)),
        }
    }
}

impl FromDbCell for i16 {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::I16(value) => Ok(*value),
            _ => Err(type_error(name, "i16", cell)),
        }
    }
}

impl FromDbCell for i32 {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::I32(value) => Ok(*value),
            _ => Err(type_error(name, "i32", cell)),
        }
    }
}

impl FromDbCell for i64 {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::I64(value) => Ok(*value),
            _ => Err(type_error(name, "i64", cell)),
        }
    }
}

impl FromDbCell for String {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::Text(value) => Ok(value.clone()),
            _ => Err(type_error(name, "String", cell)),
        }
    }
}

impl FromDbCell for Value {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::Json(value) => Ok(value.clone()),
            _ => Err(type_error(name, "serde_json::Value", cell)),
        }
    }
}

impl FromDbCell for DateTime<Utc> {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        match cell {
            DbCell::TimestampTz(value) => Ok(*value),
            _ => Err(type_error(name, "DateTime<Utc>", cell)),
        }
    }
}

impl FromDbCell for DateTime<Local> {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        let value = DateTime::<Utc>::from_cell(name, cell)?;
        Ok(value.with_timezone(&Local))
    }
}

impl<T: FromDbCell> FromDbCell for Option<T> {
    fn from_cell(name: &str, cell: &DbCell) -> Result<Self, DbError> {
        if matches!(cell, DbCell::Null) {
            return Ok(None);
        }

        T::from_cell(name, cell).map(Some)
    }
}
