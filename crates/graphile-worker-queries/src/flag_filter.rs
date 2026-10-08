use std::borrow::Cow;

use graphile_worker_database::DbValue;

/// Worker eligibility based on job-flag labels, not required capabilities.
///
/// A nonempty accepted set requires at least one matching flag. An empty set
/// adds no positive restriction. Any forbidden match vetoes a job, including
/// one that also matches acceptance. Labels match exactly and case-sensitively.
#[derive(Clone, Debug, Default)]
pub struct JobFlagFilter<'a> {
    forbidden: Cow<'a, [String]>,
    accepted: Cow<'a, [String]>,
}

impl<'a> JobFlagFilter<'a> {
    /// Borrows the forbidden and accepted label sets for a claim operation.
    pub fn new(forbidden: &'a [String], accepted: &'a [String]) -> Self {
        Self {
            forbidden: Cow::Borrowed(forbidden),
            accepted: Cow::Borrowed(accepted),
        }
    }

    pub(crate) fn shape(&self) -> (bool, bool) {
        (!self.forbidden.is_empty(), !self.accepted.is_empty())
    }

    pub(crate) fn parameter_count(&self) -> u8 {
        let (forbidden, accepted) = self.shape();
        u8::from(forbidden) + u8::from(accepted)
    }

    pub(crate) fn clause(&self, mut parameter: u8) -> String {
        let mut clause = String::new();
        if !self.forbidden.is_empty() {
            clause.push_str(&format!(
                "and ((jobs.flags ?| ${parameter}::text[]) is not true)"
            ));
            parameter += 1;
        }
        if !self.accepted.is_empty() {
            // WHERE already rejects NULL; keep the operator directly indexable.
            clause.push_str(&format!(" and (jobs.flags ?| ${parameter}::text[])"));
        }
        clause
    }

    pub(crate) fn bind(&self, params: &mut Vec<DbValue>) {
        if !self.forbidden.is_empty() {
            params.push(DbValue::TextArray(self.forbidden.to_vec()));
        }
        if !self.accepted.is_empty() {
            params.push(DbValue::TextArray(self.accepted.to_vec()));
        }
    }
}

impl JobFlagFilter<'static> {
    /// Owns the label sets, for example when creating a job stream.
    pub fn owned(forbidden: Vec<String>, accepted: Vec<String>) -> Self {
        Self {
            forbidden: Cow::Owned(forbidden),
            accepted: Cow::Owned(accepted),
        }
    }
}
