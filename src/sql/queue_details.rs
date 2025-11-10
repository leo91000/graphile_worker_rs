use std::collections::HashMap;
use std::sync::Arc;

use sqlx::{query_as, FromRow, PgPool};
use tokio::sync::RwLock;

use crate::errors::Result;

/// Cache for queue ID to queue name mappings with lazy loading.
///
/// This struct maintains an in-memory cache of queue details and fetches
/// missing entries from the database on demand. This handles the case where
/// queues are created dynamically after the worker has started.
#[derive(Debug, Clone)]
pub struct QueueDetails {
    cache: Arc<RwLock<HashMap<i32, String>>>,
    pg_pool: PgPool,
    escaped_schema: String,
}

impl QueueDetails {
    /// Get the queue name for a given queue ID.
    ///
    /// This method first checks the in-memory cache. If the queue is not found,
    /// it queries the database and updates the cache for future lookups.
    ///
    /// # Arguments
    /// * `id` - The queue ID to look up
    ///
    /// # Returns
    /// The queue name if found, or None if the queue doesn't exist
    pub async fn get(&self, id: &i32) -> Option<String> {
        // Fast path: check cache with read lock
        {
            let cache = self.cache.read().await;
            if let Some(name) = cache.get(id) {
                return Some(name.clone());
            }
        }

        // Slow path: fetch from database and update cache
        let name = self.fetch_from_db(*id).await.ok()?;

        // Update cache with write lock
        self.cache.write().await.insert(*id, name.clone());

        Some(name)
    }

    /// Fetch a queue name from the database by ID.
    ///
    /// # Arguments
    /// * `queue_id` - The queue ID to fetch
    ///
    /// # Returns
    /// The queue name if found, or an error if the query fails
    async fn fetch_from_db(&self, queue_id: i32) -> Result<String> {
        let query = format!(
            "select queue_name from {schema}._private_job_queues where id = $1",
            schema = self.escaped_schema
        );

        let name: String = sqlx::query_scalar(&query)
            .bind(queue_id)
            .fetch_one(&self.pg_pool)
            .await?;

        Ok(name)
    }
}

#[derive(FromRow)]
struct QueueRow {
    id: i32,
    queue_name: String,
}

/// Load queue details from the database.
///
/// This function loads all existing queues into the initial cache.
/// Additional queues created later will be loaded lazily on demand.
///
/// # Arguments
/// * `pg_pool` - Database connection pool
/// * `escaped_schema` - SQL-escaped schema name
///
/// # Returns
/// A QueueDetails instance with all existing queues cached
pub async fn get_queues_details(pg_pool: PgPool, escaped_schema: String) -> Result<QueueDetails> {
    let query = format!("select id, queue_name from {escaped_schema}._private_job_queues");
    let queues: Vec<QueueRow> = query_as(&query).fetch_all(&pg_pool).await?;

    let mut cache = HashMap::new();
    for row in queues {
        cache.insert(row.id, row.queue_name);
    }

    Ok(QueueDetails {
        cache: Arc::new(RwLock::new(cache)),
        pg_pool,
        escaped_schema,
    })
}
