use graphile_worker::Database;
use sqlx::postgres::PgConnectOptions;
use sqlx::PgPool;
use tokio::task::LocalSet;

use super::super::sql::safe_query;
use super::types::TestDatabase;

impl TestDatabase {
    async fn drop(&self) {
        self.test_pool.close().await;
        safe_query(format!("DROP DATABASE {} WITH (FORCE)", self.name))
            .execute(&self.source_pool)
            .await
            .expect("Failed to drop test database");
    }
}

fn test_database_url(db_url: &str, db_name: &str) -> String {
    let Ok(mut url) = url::Url::parse(db_url) else {
        return db_url.to_string();
    };

    url.set_path(db_name);
    url.to_string()
}

fn create_graphile_database(test_pool: PgPool, test_database_url: &str) -> Database {
    #[cfg(feature = "driver-tokio-postgres")]
    {
        let _ = test_pool;
        graphile_worker::tokio_postgres::TokioPostgresDatabase::from_url(test_database_url, 2)
            .expect("Failed to create tokio-postgres database")
            .into()
    }

    #[cfg(all(not(feature = "driver-tokio-postgres"), feature = "driver-sqlx"))]
    {
        let _ = test_database_url;
        test_pool.into()
    }

    #[cfg(all(not(feature = "driver-tokio-postgres"), not(feature = "driver-sqlx")))]
    {
        let _ = (test_pool, test_database_url);
        compile_error!("create_graphile_database requires a PostgreSQL driver feature");
    }
}

pub async fn create_test_database() -> TestDatabase {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut pg_conn_options: PgConnectOptions =
        db_url.parse().expect("Failed to parse DATABASE_URL");
    pg_conn_options = pg_conn_options.application_name("__test_graphile_worker");

    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(pg_conn_options.clone())
        .await
        .expect("Failed to connect to database");

    let db_id = uuid::Uuid::now_v7();
    let db_name = format!("__test_graphile_worker_{}", db_id.simple());

    safe_query(format!("CREATE DATABASE {}", db_name))
        .execute(&pg_pool)
        .await
        .expect("Failed to create test database");

    let test_options = pg_conn_options.database(&db_name);
    let test_database_url = test_database_url(&db_url, &db_name);

    let test_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect_with(test_options)
        .await
        .expect("Failed to connect to test database");
    let database = create_graphile_database(test_pool.clone(), &test_database_url);

    TestDatabase {
        source_pool: pg_pool,
        test_pool,
        database,
        name: db_name,
    }
}

pub async fn with_test_db<F, Fut>(test_fn: F)
where
    F: FnOnce(TestDatabase) -> Fut + 'static,
    Fut: std::future::Future<Output = ()>,
{
    let local_set = LocalSet::new();

    local_set
        .run_until(async move {
            let test_db = create_test_database().await;
            let test_db_2 = test_db.clone();

            let result = tokio::task::spawn_local(async move {
                test_fn(test_db_2).await;
            })
            .await;

            test_db.drop().await;
            result.expect("Test failed");
        })
        .await;
}
