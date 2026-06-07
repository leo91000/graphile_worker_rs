use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;

use crate::helpers::TestDatabase;

pub(super) fn database_url(database_name: &str) -> String {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let Ok(mut url) = url::Url::parse(&db_url) else {
        return db_url;
    };

    url.set_path(database_name);
    url.to_string()
}

pub(super) async fn connect_client(database_name: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(&database_url(database_name), NoTls)
        .await
        .expect("Failed to connect tokio-postgres client");

    drop(tokio::spawn(async move {
        let _ = connection.await;
    }));

    client
}

pub(super) fn deadpool_pool(database_name: &str) -> Pool {
    let config = database_url(database_name)
        .parse::<tokio_postgres::Config>()
        .expect("Failed to parse tokio-postgres config");
    let manager = Manager::from_config(
        config,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Pool::builder(manager)
        .max_size(2)
        .build()
        .expect("Failed to build deadpool-postgres pool")
}

pub(super) async fn count_jobs(test_db: &TestDatabase, identifier: &str) -> i64 {
    sqlx::query_scalar(
        r#"
            SELECT count(*)
            FROM graphile_worker._private_jobs jobs
            JOIN graphile_worker._private_tasks tasks ON tasks.id = jobs.task_id
            WHERE tasks.identifier = $1
        "#,
    )
    .bind(identifier)
    .fetch_one(&test_db.test_pool)
    .await
    .expect("Failed to count jobs")
}
