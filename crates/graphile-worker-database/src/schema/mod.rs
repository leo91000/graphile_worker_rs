use std::{fmt, sync::Arc};

use crate::identifier::escape_identifier;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Schema {
    escaped: Arc<str>,
}

impl Schema {
    /// Creates a schema from a raw PostgreSQL identifier.
    ///
    /// The input is escaped by this constructor, so pass raw schema names such
    /// as `graphile_worker`, not pre-escaped SQL identifiers.
    pub fn new(schema: impl AsRef<str>) -> Self {
        Self {
            escaped: Arc::from(escape_identifier(schema.as_ref())),
        }
    }

    pub fn escaped(&self) -> &str {
        self.escaped.as_ref()
    }

    pub fn identifier(&self, identifier: impl AsRef<str>) -> String {
        format!(
            "{}.{}",
            self.escaped,
            escape_identifier(identifier.as_ref())
        )
    }

    pub fn private_table(&self, table: impl AsRef<str>) -> String {
        self.identifier(format!("_private_{}", table.as_ref()))
    }

    pub fn function(&self, function: impl AsRef<str>) -> String {
        self.identifier(function)
    }
}

impl Default for Schema {
    fn default() -> Self {
        Self::new("graphile_worker")
    }
}

impl fmt::Display for Schema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.escaped())
    }
}

impl AsRef<str> for Schema {
    fn as_ref(&self) -> &str {
        self.escaped()
    }
}

impl PartialEq<&str> for Schema {
    fn eq(&self, other: &&str) -> bool {
        self.escaped() == *other
    }
}

impl PartialEq<Schema> for &str {
    fn eq(&self, other: &Schema) -> bool {
        *self == other.escaped()
    }
}

impl std::ops::Deref for Schema {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.escaped()
    }
}

impl From<&str> for Schema {
    fn from(schema: &str) -> Self {
        Self::new(schema)
    }
}

impl From<String> for Schema {
    fn from(schema: String) -> Self {
        Self::new(schema)
    }
}

impl From<&String> for Schema {
    fn from(schema: &String) -> Self {
        Self::new(schema)
    }
}

impl From<&Schema> for Schema {
    fn from(schema: &Schema) -> Self {
        schema.clone()
    }
}

#[cfg(test)]
mod tests;
