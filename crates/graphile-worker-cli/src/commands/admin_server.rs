use anyhow::{Context, Result};
use graphile_worker::{Schema, WorkerUtils};
use graphile_worker_admin_ui::AdminServerConfig;
use sqlx::PgPool;

use crate::admin::{build_admin_auth, print_admin_startup};
use crate::args::{AdminArgs, Cli};

pub(crate) async fn serve(
    cli: &Cli,
    args: &AdminArgs,
    pool: &PgPool,
    utils: &WorkerUtils,
    schema: &Schema,
) -> Result<()> {
    let auth = build_admin_auth(args)?;
    print_admin_startup(args, &auth);
    let config = AdminServerConfig::builder(pool.clone(), utils.clone())
        .schema(schema)
        .schema_name(cli.schema.clone())
        .listen_addr(args.listen)
        .auth(auth)
        .read_only(args.read_only)
        .build()?;

    graphile_worker_admin_ui::serve(config)
        .await
        .context("admin UI server stopped with an error")
}
