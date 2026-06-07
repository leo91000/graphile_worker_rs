use super::DbValue;

#[derive(Clone, Debug, Default)]
pub struct DbParams(Vec<DbValue>);

impl DbParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: DbValue) {
        self.0.push(value);
    }

    pub fn values(&self) -> &[DbValue] {
        &self.0
    }
}

impl From<Vec<DbValue>> for DbParams {
    fn from(value: Vec<DbValue>) -> Self {
        Self(value)
    }
}
