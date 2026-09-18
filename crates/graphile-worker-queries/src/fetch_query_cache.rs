use std::{cell::RefCell, collections::HashMap, sync::Arc};

use graphile_worker_database::Schema;

// SQL text only: never cache task IDs, flags, timestamps, results or connections.
// Bound each runtime thread's cache for applications using tenant schemas.
const CAPACITY: usize = 128;
type Key = (Schema, bool, bool, bool);
thread_local! {
    static QUERIES: RefCell<HashMap<Key, Arc<str>>> = RefCell::default();
}

/// Reuses SQL text for one schema and fetch shape on the current thread.
///
/// Bound values are never cached. Clearing at capacity bounds memory retained
/// by applications that visit many tenant schemas.
pub(crate) fn fetch_query(
    schema: &Schema,
    flags: bool,
    local_time: bool,
    batch: bool,
    construct: impl FnOnce() -> String,
) -> Arc<str> {
    QUERIES.with(|queries| {
        let mut queries = queries.borrow_mut();
        let key = (schema.clone(), flags, local_time, batch);
        if let Some(sql) = queries.get(&key) {
            return Arc::clone(sql);
        }
        let sql: Arc<str> = construct().into();
        if queries.len() == CAPACITY {
            queries.clear();
        }
        queries.insert(key, Arc::clone(&sql));
        sql
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards against cross-tenant query reuse and unbounded thread-local retention.
    #[test]
    fn caches_each_shape_and_schema_without_retaining_unbounded_tenants() {
        for schema in [Schema::default(), Schema::new("quoted\"schema")] {
            for flags in [false, true] {
                for local_time in [false, true] {
                    for batch in [false, true] {
                        let expected = format!("{schema}/{flags}/{local_time}/{batch}");
                        let first =
                            fetch_query(&schema, flags, local_time, batch, || expected.clone());
                        let second =
                            fetch_query(&schema, flags, local_time, batch, || panic!("cache miss"));
                        assert_eq!(&*first, expected);
                        assert!(Arc::ptr_eq(&first, &second));
                    }
                }
            }
        }
        for tenant in 0..CAPACITY * 2 {
            fetch_query(
                &Schema::new(format!("tenant_{tenant}")),
                false,
                false,
                false,
                || tenant.to_string(),
            );
        }
        QUERIES.with(|queries| assert!(queries.borrow().len() <= CAPACITY));
    }
}
