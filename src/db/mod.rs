pub mod models;

use sqlx::{MySql, MySqlPool, Pool};
use tokio::sync::OnceCell;

pub static TOKEN: once_cell::sync::OnceCell<String> = once_cell::sync::OnceCell::new();
pub static DATABASE_URL: once_cell::sync::OnceCell<String> = once_cell::sync::OnceCell::new();
pub static DATABASE_URL_TOKIDM: once_cell::sync::OnceCell<String> =
    once_cell::sync::OnceCell::new();

static POOL: OnceCell<MySqlPool> = OnceCell::const_new();
static POOL_TOKIDM: OnceCell<MySqlPool> = OnceCell::const_new();

pub async fn get_pool() -> Result<&'static MySqlPool, sqlx::Error> {
    POOL.get_or_try_init(|| async { Pool::<MySql>::connect(DATABASE_URL.get().unwrap()).await })
        .await
}

pub async fn get_pool_tokidm() -> Result<&'static MySqlPool, sqlx::Error> {
    POOL_TOKIDM
        .get_or_try_init(|| async {
            Pool::<MySql>::connect(DATABASE_URL_TOKIDM.get().unwrap()).await
        })
        .await
}

pub fn get_row_u64(row: &sqlx::mysql::MySqlRow, col: &str) -> Option<u64> {
    use sqlx::Row;
    row.try_get::<u64, _>(col)
        .or_else(|_| row.try_get::<i64, _>(col).map(|v| v as u64))
        .or_else(|_| row.try_get::<u32, _>(col).map(|v| v as u64))
        .or_else(|_| row.try_get::<i32, _>(col).map(|v| v as u64))
        .ok()
}

pub fn get_row_opt_u64(row: &sqlx::mysql::MySqlRow, col: &str) -> Option<u64> {
    use sqlx::Row;
    row.try_get::<Option<u64>, _>(col)
        .or_else(|_| {
            row.try_get::<Option<i64>, _>(col)
                .map(|o| o.map(|v| v as u64))
        })
        .or_else(|_| {
            row.try_get::<Option<u32>, _>(col)
                .map(|o| o.map(|v| v as u64))
        })
        .or_else(|_| {
            row.try_get::<Option<i32>, _>(col)
                .map(|o| o.map(|v| v as u64))
        })
        .ok()
        .flatten()
}

pub fn get_row_datetime(
    row: &sqlx::mysql::MySqlRow,
    col: &str,
) -> Option<chrono::DateTime<chrono::Utc>> {
    use sqlx::Row;
    row.try_get::<chrono::DateTime<chrono::Utc>, _>(col)
        .or_else(|_| {
            row.try_get::<chrono::NaiveDateTime, _>(col)
                .map(|ndt| ndt.and_utc())
        })
        .or_else(|_| {
            row.try_get::<chrono::DateTime<chrono::Local>, _>(col)
                .map(|dt| dt.with_timezone(&chrono::Utc))
        })
        .ok()
}
