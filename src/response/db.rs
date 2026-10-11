use serde::Serialize;
use sqlx::{MySqlPool, Row};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::get_row_u64;

#[derive(Clone, Serialize)]
pub struct Resp {
    pub values: Vec<String>,
    pub probs: Vec<String>,
}

pub struct RespStore {
    inner: Arc<Mutex<Vec<Resp>>>,
}

impl RespStore {
    pub fn new(initial: Vec<Resp>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(initial)),
        }
    }

    pub fn shared(&self) -> Arc<Mutex<Vec<Resp>>> {
        self.inner.clone()
    }

    pub async fn update(&self, new_data: Vec<Resp>) {
        let mut guard = self.inner.lock().await;
        *guard = new_data;
    }

    pub async fn load_all(pool: &MySqlPool) -> Result<Vec<Resp>, sqlx::Error> {
        let rows = sqlx::query("SELECT value_json, prob_json FROM resp ORDER BY id")
            .fetch_all(pool)
            .await?;

        let mut out = Vec::with_capacity(rows.len());

        for row in rows {
            let value_json: String = row.get("value_json");
            let prob_json: String = row.get("prob_json");
            let values: Vec<String> = serde_json::from_str(&value_json).unwrap();
            let probs: Vec<String> = serde_json::from_str(&prob_json).unwrap();

            out.push(Resp { values, probs });
        }

        Ok(out)
    }
}

pub async fn db_new_key(pool: &MySqlPool, key: &str) -> Result<(), sqlx::Error> {
    let rows = sqlx::query("SELECT value_json FROM resp")
        .fetch_all(pool)
        .await?;

    for r in rows {
        let value_json: String = r.get("value_json");
        let values: Vec<String> = serde_json::from_str(&value_json)
            .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error: {}", e)))?;

        if values.contains(&key.to_string()) {
            return Err(sqlx::Error::Protocol("Key already exists".into()));
        }
    }

    let value_json = serde_json::to_string(&[key])
        .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;

    let prob_json = "[]";

    sqlx::query("INSERT INTO resp (value_json, prob_json) VALUES (?, ?)")
        .bind(value_json)
        .bind(prob_json)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn db_add_resp(pool: &MySqlPool, key: &str, resp: &str) -> Result<(), sqlx::Error> {
    db_add_resp_bulk(pool, key, &[resp.to_string()]).await
}

pub async fn db_add_resp_bulk(
    pool: &MySqlPool,
    key: &str,
    new_resps: &[String],
) -> Result<(), sqlx::Error> {
    let rows = sqlx::query("SELECT id, value_json, prob_json FROM resp")
        .fetch_all(pool)
        .await?;

    for r in rows {
        let value_json: String = r.get("value_json");
        let values: Vec<String> = serde_json::from_str(&value_json)
            .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error (value): {}", e)))?;

        if values.contains(&key.to_string()) {
            let id: u64 = get_row_u64(&r, "id").unwrap_or_default();
            let prob_json: String = r.get("prob_json");
            let mut probs: Vec<String> = serde_json::from_str(&prob_json)
                .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error (prob): {}", e)))?;

            probs.extend(new_resps.iter().cloned());

            let new_probs_json = serde_json::to_string(&probs)
                .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;

            sqlx::query("UPDATE resp SET prob_json = ? WHERE id = ?")
                .bind(new_probs_json)
                .bind(id)
                .execute(pool)
                .await?;

            return Ok(());
        }
    }

    Err(sqlx::Error::Protocol("Key not found".into()))
}

pub async fn db_add_key(pool: &MySqlPool, key: &str, new_key: &str) -> Result<(), sqlx::Error> {
    db_add_key_bulk(pool, key, &[new_key.to_string()]).await
}

pub async fn db_add_key_bulk(
    pool: &MySqlPool,
    key: &str,
    new_keys: &[String],
) -> Result<(), sqlx::Error> {
    let rows = sqlx::query("SELECT id, value_json FROM resp")
        .fetch_all(pool)
        .await?;

    for r in rows {
        let value_json: String = r.get("value_json");
        let mut values: Vec<String> = serde_json::from_str(&value_json)
            .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error: {}", e)))?;

        if values.contains(&key.to_string()) {
            if new_keys.iter().any(|k| values.contains(k)) {
                return Err(sqlx::Error::Protocol("Duplicate key in new keys".into()));
            }

            let id: u64 = get_row_u64(&r, "id").unwrap_or_default();
            values.extend(new_keys.iter().cloned());

            let new_values_json = serde_json::to_string(&values)
                .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;

            sqlx::query("UPDATE resp SET value_json = ? WHERE id = ?")
                .bind(new_values_json)
                .bind(id)
                .execute(pool)
                .await?;

            return Ok(());
        }
    }

    Err(sqlx::Error::Protocol("Original key not found".into()))
}

pub async fn db_get_table(pool: &MySqlPool, key: &str) -> Result<Option<Resp>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, value_json, prob_json FROM resp")
        .fetch_all(pool)
        .await?;

    for r in rows {
        let value_json: String = r.get("value_json");
        let values: Vec<String> = serde_json::from_str(&value_json)
            .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error (value): {}", e)))?;

        if values.contains(&key.to_string()) {
            let prob_json: String = r.get("prob_json");
            let probs: Vec<String> = serde_json::from_str(&prob_json)
                .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error (prob): {}", e)))?;

            return Ok(Some(Resp { values, probs }));
        }
    }

    Ok(None)
}

pub async fn db_remove_table(pool: &MySqlPool, key: &str, force: bool) -> Result<(), sqlx::Error> {
    // 1. 데이터 조회
    let rows = sqlx::query("SELECT id, value_json FROM resp")
        .fetch_all(pool)
        .await?;

    for r in rows {
        let value_json: String = r.get("value_json");
        let values: Vec<String> = serde_json::from_str(&value_json)
            .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error: {}", e)))?;

        if values.contains(&key.to_string()) {
            // 3. 비즈니스 로직 검증
            if values.len() > 1 && !force {
                return Err(sqlx::Error::Protocol(
                    "Cannot remove: multiple keys in this record (use force=true to override)"
                        .into(),
                ));
            }

            // 4. 데이터 삭제
            let id: u64 = get_row_u64(&r, "id").unwrap_or_default();
            sqlx::query("DELETE FROM resp WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await?;

            return Ok(());
        }
    }

    Err(sqlx::Error::Protocol("Key not found".into()))
}

pub async fn db_remove_resp(
    pool: &MySqlPool,
    key: &str,
    targets: &str,
    multi: bool,
) -> Result<(), sqlx::Error> {
    let rows = sqlx::query("SELECT id, value_json, prob_json FROM resp")
        .fetch_all(pool)
        .await?;

    for r in rows {
        let value_json: String = r.get("value_json");
        let values: Vec<String> = serde_json::from_str(&value_json)
            .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error (value): {}", e)))?;

        if values.contains(&key.to_string()) {
            let id: u64 = get_row_u64(&r, "id").unwrap_or_default();
            let prob_json: String = r.get("prob_json");
            let mut probs: Vec<String> = serde_json::from_str(&prob_json)
                .map_err(|e| sqlx::Error::Protocol(format!("JSON parse error (prob): {}", e)))?;

            if multi {
                probs.retain(|p| p != targets);
            } else if let Some(pos) = probs.iter().position(|p| p == targets) {
                probs.remove(pos);
            }

            if probs.is_empty() {
                sqlx::query("DELETE FROM resp WHERE id = ?")
                    .bind(id)
                    .execute(pool)
                    .await?;
            } else {
                let probs_json = serde_json::to_string(&probs)
                    .map_err(|e| sqlx::Error::Protocol(format!("Serialization error: {}", e)))?;

                sqlx::query("UPDATE resp SET prob_json = ? WHERE id = ?")
                    .bind(probs_json)
                    .bind(id)
                    .execute(pool)
                    .await?;
            }

            return Ok(());
        }
    }

    Err(sqlx::Error::Protocol("Key not found".into()))
}
