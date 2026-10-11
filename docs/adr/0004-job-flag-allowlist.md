# Optional job-flag allowlist

Workers may restrict claims to jobs whose every flag key belongs to an explicit
allowlist. This supports build pools with an expanding set of architecture and
environment labels without enumerating every incompatible label as an exclusion.
Accepted flags retain any-match routing and forbidden flags veto matches; these
rules combine with the allowlist rather than changing its meaning.

An unset allowlist preserves current behavior. An explicitly empty allowlist
admits only NULL or empty-object flags for this predicate. Every key must be
listed, including operational labels such as `infrastructure_resilient`; no
capability classification or automatic exemptions are inferred. Matching is exact
and case-sensitive and ignores values. Enabled allowlists reject non-object JSON
flags; legacy workers retain their existing behavior.

Keep the upstream schema and producer interfaces unchanged. This is per-worker
filtering, so old or unconfigured workers and previously prefetched work remain
governed by their own settings. Drain those workers before relying on the policy,
or isolate new work using task identifiers registered only by configured workers.
A separate requirements/capabilities model remains deferred.

The shared immutable filter reaches every claim path, including LocalQueue batch
prefetch and named-queue follow-up claims. Filtering precedes candidate limits,
job ownership changes and attempt increments. Existing queue subqueries can still
take temporary PostgreSQL row locks; persisted ownership is a separate concern.
