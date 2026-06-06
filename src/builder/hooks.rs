use std::fmt::Debug;

use graphile_worker_lifecycle_hooks::{Event, Plugin};

use super::WorkerOptions;

impl WorkerOptions {
    /// Adds a custom extension to the worker context.
    ///
    /// Extensions provide custom application state or dependencies
    /// that are accessible from task handlers. Useful for sharing resources
    /// like API clients, configuration, or other state between jobs.
    ///
    /// # Type Parameters
    /// * `T` - Type of the extension (must be Clone, Send, Sync, Debug, and 'static)
    ///
    /// # Arguments
    /// * `value` - The extension instance to add
    ///
    /// # Example
    /// ```
    /// # use graphile_worker::WorkerOptions;
    /// # use std::sync::Arc;
    /// #
    /// # #[derive(Clone, Debug)]
    /// # struct AppConfig {
    /// #     api_key: String,
    /// # }
    /// #
    /// # #[derive(Clone, Debug)]
    /// # struct Database {
    /// #     // Database connection or client
    /// # }
    ///
    /// let config = AppConfig {
    ///     api_key: "secret".to_string(),
    /// };
    ///
    /// let db = Database {
    ///     // Initialize database
    /// };
    ///
    /// let options = WorkerOptions::default()
    ///     .add_extension(config)
    ///     .add_extension(db);
    /// ```
    pub fn add_extension<T: Clone + Send + Sync + Debug + 'static>(mut self, value: T) -> Self {
        self.extensions.insert(value);
        self
    }

    /// Registers an event handler for a specific lifecycle event.
    ///
    /// This method allows registering individual handlers for specific events
    /// without creating a full plugin. The handler is a closure that receives
    /// the event context and returns the appropriate output type.
    ///
    /// # Type Parameters
    /// * `E` - The event type to handle
    ///
    /// # Arguments
    /// * `event` - The event marker (e.g., `JobStart`, `BeforeJobRun`)
    /// * `handler` - The async handler closure
    ///
    /// # Example
    /// ```ignore
    /// use graphile_worker::{WorkerOptions, JobStart, BeforeJobRun, HookResult};
    ///
    /// let options = WorkerOptions::default()
    ///     .on(JobStart, |ctx| async move {
    ///         println!("Job {} starting", ctx.job.id());
    ///     })
    ///     .on(BeforeJobRun, |ctx| async move {
    ///         if ctx.payload.get("skip").and_then(|v| v.as_bool()).unwrap_or(false) {
    ///             return HookResult::Skip;
    ///         }
    ///         HookResult::Continue
    ///     });
    /// ```
    pub fn on<E, F, Fut>(mut self, event: E, handler: F) -> Self
    where
        E: Event,
        F: Fn(E::Context) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = E::Output> + Send + 'static,
    {
        self.hooks.on(event, handler);
        self
    }

    /// Adds a lifecycle hook plugin to the worker.
    ///
    /// Plugins can observe and intercept various worker events such as
    /// job execution, worker startup/shutdown, and cron scheduling.
    /// Multiple plugins can be registered and they execute in registration order.
    ///
    /// # Type Parameters
    /// * `P` - A type implementing the Plugin trait
    ///
    /// # Arguments
    /// * `plugin` - The plugin instance to add
    ///
    /// # Example
    /// ```ignore
    /// use graphile_worker::{WorkerOptions, Plugin, HookRegistry, JobStart};
    ///
    /// struct LoggingPlugin;
    ///
    /// impl Plugin for LoggingPlugin {
    ///     fn register(self, hooks: &mut HookRegistry) {
    ///         hooks.on::<JobStart>(|ctx| async move {
    ///             println!("Job {} starting", ctx.job.id());
    ///         });
    ///     }
    /// }
    ///
    /// let options = WorkerOptions::default()
    ///     .add_plugin(LoggingPlugin);
    /// ```
    pub fn add_plugin<P: Plugin>(mut self, plugin: P) -> Self {
        plugin.register(&mut self.hooks);
        self
    }
}
