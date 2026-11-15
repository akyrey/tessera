//! SQLite connection pool and migrations.

use std::str::FromStr;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

const MAX_CONNECTIONS: u32 = 5;
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("cannot open database: {0}")]
    Connect(#[from] sqlx::Error),
    #[error("cannot run migrations: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

/// Opens the pool (creating the file if needed) and applies pending migrations.
///
/// WAL mode is required by Litestream, which replicates the database to R2.
pub async fn connect(database_url: &str) -> Result<SqlitePool, DbError> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(BUSY_TIMEOUT)
        // Sorts and temp indexes stay in RAM: the container root filesystem is read-only.
        .pragma("temp_store", "MEMORY");

    let pool = SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Cheap round-trip used by the readiness probe.
pub async fn ping(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await.map(|_| ())
}
