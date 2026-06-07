use crate::{DbCell, DbRow};

pub fn cells(values: impl IntoIterator<Item = (impl Into<String>, DbCell)>) -> DbRow {
    DbRow::new(
        values
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
    )
}
