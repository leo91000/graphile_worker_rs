use tokio::sync::{Mutex, OnceCell};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub struct StaticCounter {
    cell: OnceCell<Mutex<u32>>,
}

async fn init_job_count() -> Mutex<u32> {
    Mutex::new(0)
}

impl StaticCounter {
    pub const fn new() -> Self {
        Self {
            cell: OnceCell::const_new(),
        }
    }

    pub async fn increment(&self) -> u32 {
        let cell = self.cell.get_or_init(init_job_count).await;
        let mut count = cell.lock().await;
        *count += 1;
        *count
    }

    pub async fn get(&self) -> u32 {
        let cell = self.cell.get_or_init(init_job_count).await;
        *cell.lock().await
    }

    pub async fn reset(&self) {
        let cell = self.cell.get_or_init(init_job_count).await;
        let mut count = cell.lock().await;
        *count = 0;
    }
}

impl Default for StaticCounter {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn enable_logs() {
    static ONCE: OnceCell<()> = OnceCell::const_new();

    ONCE.get_or_init(|| async {
        let fmt_layer = tracing_subscriber::fmt::layer();
        // Log level set to debug except for sqlx set at warn (to not show all sql requests)
        let filter_layer = EnvFilter::try_new("debug,sqlx=warn").unwrap();

        tracing_subscriber::registry()
            .with(filter_layer)
            .with(fmt_layer)
            .init();
    })
    .await;
}
