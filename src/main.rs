mod commands;
mod constants;
mod db;
mod dm;
mod handler;
mod macros;
mod response;
mod tasks;
mod upboard;
mod utils;

#[cfg(test)]
mod tests;

use dotenvy::dotenv;
use serenity::all::GatewayIntents;
use serenity::prelude::Client;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

use db::{
    get_pool, get_pool_tokidm, get_row_opt_u64, get_row_u64, DATABASE_URL, DATABASE_URL_TOKIDM,
    TOKEN,
};
use handler::Handler;
use response::RespStore;

#[tokio::main]
async fn main() {
    dotenv().ok();

    if let Err(e) =
        TOKEN.set(env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set in .env file"))
    {
        eprintln!("[ERROR] TOKEN set(): {:?}", e);
        return;
    }
    if let Err(e) =
        DATABASE_URL.set(env::var("DATABASE_URL").expect("DATABASE_URL not set in .env file"))
    {
        eprintln!("[ERROR] DATABASE_URL set(): {:?}", e);
        return;
    }
    if let Err(e) = DATABASE_URL_TOKIDM
        .set(env::var("DATABASE_URL_TOKIDM").expect("DATABASE_URL_TOKIDM not set in .env file"))
    {
        eprintln!("[ERROR] DATABASE_URL_TOKIDM set(): {:?}", e);
        return;
    }

    let intents =
          GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGE_REACTIONS
        | GatewayIntents::GUILD_MESSAGE_TYPING
        | GatewayIntents::DIRECT_MESSAGE_TYPING
        /*
        | GatewayIntents::GUILD_MESSAGE_POLLS
        | GatewayIntents::DIRECT_MESSAGE_POLLS
        */
        ;

    let pool = get_pool().await.expect("[ERROR] Database connect()");

    let initial_resp_vec = RespStore::load_all(pool).await.unwrap_or_else(|e| {
        eprintln!("[ERROR] load_all_resp(): {:?}", e);
        Vec::new()
    });

    let resp_store = Arc::new(RespStore::new(initial_resp_vec));

    let blacklist_guild_raw = sqlx::query("SELECT id FROM message_blacklist_guild")
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| {
            eprintln!("[ERROR] load_all message_blacklist_guild: {:?}", e);
            Vec::new()
        });

    let blacklist_guilds = Arc::new(Mutex::new(
        blacklist_guild_raw
            .iter()
            .filter_map(|row| get_row_u64(row, "id"))
            .collect::<Vec<u64>>(),
    ));

    let dm_channels: Arc<Mutex<Vec<(u64, u64)>>> = {
        match get_pool_tokidm().await {
            Err(e) => {
                eprintln!("[ERROR] Database connect() DM: {:?}", e);
                Arc::new(Mutex::new(Vec::new()))
            }
            Ok(pool_dm) => {
                let rows = sqlx::query("SHOW TABLES")
                    .fetch_all(pool_dm)
                    .await
                    .unwrap_or_default();

                Arc::new(Mutex::new(
                    rows.iter()
                        .filter_map(|r| {
                            let name: String = r.try_get(0).ok()?;

                            name.strip_prefix("c")
                                .and_then(|s| s.split_once('u'))
                                .and_then(|(bot, user)| {
                                    Some((bot.parse::<u64>().ok()?, user.parse::<u64>().ok()?))
                                })
                        })
                        .collect(),
                ))
            }
        }
    };

    let trap_ids = {
        let rows = sqlx::query("SELECT guild_id, channel_id, log_id FROM trap")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        Arc::new(Mutex::new(
            rows.iter()
                .filter_map(|r| {
                    let guild_id = get_row_u64(r, "guild_id")?;
                    let channel_id = get_row_u64(r, "channel_id")?;
                    let log_id = get_row_opt_u64(r, "log_id");
                    Some((guild_id, (channel_id, log_id)))
                })
                .collect::<HashMap<u64, (u64, Option<u64>)>>(),
        ))
    };

    let xfix_channels = {
        if let Err(e) = sqlx::query(
            "CREATE TABLE IF NOT EXISTS xfix_guild (guild_id BIGINT UNSIGNED NOT NULL PRIMARY KEY)",
        )
        .execute(pool)
        .await
        {
            eprintln!("[ERROR] xfix_guild create table: {:?}", e);
        }
        let rows = sqlx::query("SELECT guild_id FROM xfix_guild")
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        Arc::new(Mutex::new(
            rows.iter()
                .filter_map(|r| get_row_u64(r, "guild_id"))
                .collect::<HashSet<u64>>(),
        ))
    };

    let mut client = Client::builder(TOKEN.get().unwrap(), intents)
        .event_handler(Handler {
            is_ready: Arc::new(AtomicBool::new(false)),
            delete_messages: Arc::new(Mutex::new(Vec::new())),
            stop_flag: Arc::new(AtomicBool::new(false)),
            stop_notify: Arc::new(Notify::new()),
            resp_store: resp_store.clone(),
            resp: resp_store.shared(),
            blacklist_guilds,
            dm_channels,
            trap_ids,
            xfix_channels,
        })
        .await
        .expect("[ERROR] Client builder()");

    if let Err(e) = client.start().await {
        eprintln!("[ERROR] Client start(): {e:?}");
    }
}
