use std::borrow::Cow;

use graphile_worker_database::DbValue;

/// Worker eligibility based on job-flag labels, not required capabilities.
///
/// A nonempty accepted set requires at least one matching flag. An empty set
/// adds no positive restriction. Any forbidden match vetoes a job, including
/// one that also matches acceptance. Labels match exactly and case-sensitively.
/// An optional allowlist requires every job flag key to be listed, including
/// operational flags. An explicitly empty allowlist admits only untagged jobs
/// for this predicate; accepted and forbidden filters still apply.
#[derive(Clone, Debug, Default)]
pub struct JobFlagFilter<'a> {
    forbidden: Cow<'a, [String]>,
    accepted: Cow<'a, [String]>,
    allowed: Option<Cow<'a, [String]>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FlagFilterShape {
    pub forbidden: bool,
    pub accepted: bool,
    pub allowlist: bool,
}

impl<'a> JobFlagFilter<'a> {
    /// Borrows the forbidden and accepted label sets for a claim operation.
    pub fn new(forbidden: &'a [String], accepted: &'a [String]) -> Self {
        Self {
            forbidden: Cow::Borrowed(forbidden),
            accepted: Cow::Borrowed(accepted),
            allowed: None,
        }
    }

    /// Restricts claims to jobs whose every flag key belongs to this set.
    ///
    /// An empty set permits only NULL or empty-object flags for this predicate.
    /// Keys match exactly; values are ignored. Non-object JSON flags are rejected.
    /// Accepted routing and forbidden vetoes still apply. Without this method,
    /// no allowlist restriction is added.
    pub fn with_allowlist(mut self, flags: impl Into<Cow<'a, [String]>>) -> Self {
        self.allowed = Some(flags.into());
        self
    }

    /// Returns the configured allowlist, distinguishing unset from empty.
    pub fn allowlist(&self) -> Option<&[String]> {
        self.allowed.as_deref()
    }

    /// Borrows this filter's labels without cloning an owned configuration.
    pub fn as_borrowed(&self) -> JobFlagFilter<'_> {
        JobFlagFilter {
            forbidden: Cow::Borrowed(&self.forbidden),
            accepted: Cow::Borrowed(&self.accepted),
            allowed: self.allowed.as_deref().map(Cow::Borrowed),
        }
    }

    pub(crate) fn shape(&self) -> FlagFilterShape {
        FlagFilterShape {
            forbidden: !self.forbidden.is_empty(),
            accepted: !self.accepted.is_empty(),
            allowlist: self.allowed.is_some(),
        }
    }

    pub(crate) fn parameter_count(&self) -> u8 {
        let shape = self.shape();
        u8::from(shape.forbidden) + u8::from(shape.accepted) + u8::from(shape.allowlist)
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
            parameter += 1;
        }
        if self.allowed.is_some() {
            // CASE protects the subtraction operator from scalar JSON inputs.
            clause.push_str(&format!(
                " and (case when jobs.flags is null then true when jsonb_typeof(jobs.flags) = 'object' then (jobs.flags - ${parameter}::text[]) = '{{}}'::jsonb else false end)"
            ));
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
        if let Some(allowed) = &self.allowed {
            params.push(DbValue::TextArray(allowed.to_vec()));
        }
    }
}

impl JobFlagFilter<'static> {
    /// Owns the label sets, for example when creating a job stream.
    pub fn owned(forbidden: Vec<String>, accepted: Vec<String>) -> Self {
        Self {
            forbidden: Cow::Owned(forbidden),
            accepted: Cow::Owned(accepted),
            allowed: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Preserve the legacy clause and parameter order when allowlisting is unset.
    #[test]
    fn unset_allowlist_keeps_legacy_sql_and_bindings() {
        let forbidden = vec!["gpu".to_owned()];
        let accepted = vec!["linux".to_owned()];
        for start in [3, 4] {
            for (skip, accept) in [(false, false), (true, false), (false, true), (true, true)] {
                let filter = JobFlagFilter::new(
                    if skip { &forbidden } else { &[] },
                    if accept { &accepted } else { &[] },
                );
                let expected = match (skip, accept) {
                    (false, false) => String::new(),
                    (true, false) => format!("and ((jobs.flags ?| ${start}::text[]) is not true)"),
                    (false, true) => format!(" and (jobs.flags ?| ${start}::text[])"),
                    (true, true) => format!("and ((jobs.flags ?| ${start}::text[]) is not true) and (jobs.flags ?| ${}::text[])", start + 1),
                };
                assert_eq!(filter.clause(start), expected);
                let mut params = Vec::new();
                filter.bind(&mut params);
                let expected = [skip.then_some(&forbidden), accept.then_some(&accepted)]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                assert_eq!(params.len(), expected.len());
                assert_eq!(filter.parameter_count() as usize, params.len());
                for (value, expected) in params.iter().zip(expected) {
                    let DbValue::TextArray(value) = value else {
                        panic!("expected text-array binding")
                    };
                    assert_eq!(value, expected);
                }
            }
        }
    }
}
