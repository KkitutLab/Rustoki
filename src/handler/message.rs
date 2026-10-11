use chrono::Local;
use serenity::all::{
    ActivityData, ChannelId, ChannelType, CreateChannel, CreateEmbed, CreateEmbedFooter,
    CreateMessage, Message, OnlineStatus,
};
use serenity::prelude::Context;
use sqlx::{MySql, QueryBuilder};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tokio::time::sleep;

use crate::commands::rtcmd::handle_rtcmd;
use crate::commands::slash::trap::TrapIdsStore;
use crate::constants::{BOT_USERID, CHANNELID_LOG_SECRET, GUILD_ID, KKITUT_USERID};
use crate::db::{get_pool, get_pool_tokidm};
use crate::dm::{find_dm_either_channel, find_dm_user_channel, relay_dm_message};
use crate::macros::er;
use crate::response::{get_response, get_special_response, Resp, RespStore};

pub fn extract_call_query(content: &str) -> Option<String> {
    let upmsg = content.to_uppercase();
    if let Some(stripped) = upmsg.strip_prefix("러스토키 ") {
        let trimmed = stripped.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    } else if let Some(stripped) = upmsg.strip_prefix("<\" ") {
        let trimmed = stripped.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

pub async fn execute_bot_call(
    ctx: &Context,
    msg: &Message,
    query: &str,
    resp: &Arc<Mutex<Vec<Resp>>>,
    is_my_server: bool,
) -> Option<String> {
    if query == "나" || query == "ME" {
        if let Some(create_message) = get_response(resp, &format!("<@{}>", msg.author.id)).await {
            if let Err(e) = msg.channel_id.say(&ctx.http, &create_message).await {
                eprintln!("[ERROR] send_message(): {:?}", e);
            }
            Some(create_message)
        } else {
            let formatted = format!("{}{}", msg.author.name, "은/는 DB에 없어");
            if let Err(e) = msg.channel_id.say(&ctx.http, &formatted).await {
                eprintln!("[ERROR] ? say(): {:?}", e);
            }
            Some(formatted)
        }
    } else if let Some(create_message) = get_response(resp, query).await {
        if let Err(e) = msg.channel_id.say(&ctx.http, &create_message).await {
            eprintln!("[ERROR] send_message(): {:?}", e);
        }
        Some(create_message)
    } else if let Some(spmsg) = get_special_response(query, ctx).await {
        if spmsg.extra_note.starts_with("*sticker") && !is_my_server {
            let note = "(여기선 스티커를 보낼 수가 없어)";
            if let Err(e) = msg
                .channel_id
                .send_message(
                    &ctx.http,
                    CreateMessage::new()
                        .embed(CreateEmbed::new().description(note).color(0xFFAAAA)),
                )
                .await
            {
                eprintln!("[ERROR] send_message(): {:?}", e);
            }
            Some(note.to_string())
        } else {
            let note = spmsg.extra_note.clone();
            if let Err(e) = msg
                .channel_id
                .send_message(&ctx.http, spmsg.to_create_message())
                .await
            {
                eprintln!("[ERROR] send_message(): {:?}", e);
            }
            Some(note)
        }
    } else {
        if let Err(e) = msg.channel_id.say(&ctx.http, "?").await {
            eprintln!("[ERROR] say(): {:?}", e);
        }
        None
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_message(
    ctx: &Context,
    msg: Message,
    resp: &Arc<Mutex<Vec<Resp>>>,
    resp_store: &Arc<RespStore>,
    blacklist_guilds: &Arc<Mutex<Vec<u64>>>,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
    trap_ids: &TrapIdsStore,
    stop_flag: &Arc<AtomicBool>,
    stop_notify: &Arc<Notify>,
) {
    let localnow = Local::now();
    if msg.author.id == BOT_USERID {
        return;
    }
    /*
    if bot_userid_u64 == KKITUT_USERID && msg.content.starts_with("<#") {
        return;
    }
    */

    if let Some(guild_id) = msg.guild_id {
        let is_trap = {
            let lock = trap_ids.lock().await;
            lock.get(&guild_id.get()).and_then(|(channel_id, log_id)| {
                if msg.channel_id == ChannelId::new(*channel_id) {
                    Some((*channel_id, *log_id))
                } else {
                    None
                }
            })
        };

        if let Some((_, trap_log)) = is_trap {
            if let Some(perms) = msg.author_permissions(&ctx.cache) {
                if perms.administrator() {
                    return;
                }
            }

            let user_id = msg.author.id.get();
            let embedfooter = CreateEmbedFooter::new(&msg.author.name).icon_url(
                msg.author
                    .avatar_url()
                    .unwrap_or_else(|| msg.author.default_avatar_url()),
            );
            let ban_time = Local::now();
            if let Err(e) = guild_id.ban(&ctx.http, msg.author.id, 1).await {
                eprintln!("[ERROR] Trap ban(): {:?}", e);
                if let Some(log) = trap_log {
                    let embed = CreateEmbed::new()
                        .title("FAIL : Trap Ban")
                        .description(format!("{:?}", e))
                        .color(0xFFAAAA);
                    if let Err(e) = ChannelId::new(log)
                        .send_message(&ctx.http, CreateMessage::new().embed(embed))
                        .await
                    {
                        eprintln!("[ERROR] Trap error send_message(): {e:?}");
                    }
                }
                return;
            }

            if let Some(log) = trap_log {
                let embed = CreateEmbed::new()
                    .title("GOTCHA")
                    .description(format!("<@{}>", user_id))
                    .color(0xFFAAAA)
                    .footer(embedfooter.clone())
                    .timestamp(ban_time);

                let builder = CreateMessage::new().embed(embed);
                if let Err(e) = ChannelId::new(log).send_message(&ctx.http, builder).await {
                    eprintln!("[ERROR] Trap send_message(): {:?}", e);
                }
            }
            return;
        }

        let is_my_server = guild_id == GUILD_ID.get();

        let has_blacklist = {
            let guard = blacklist_guilds.lock().await;
            guard.contains(&guild_id.get())
        };

        if !has_blacklist {
            if let Some(query) = extract_call_query(&msg.content) {
                let _ = execute_bot_call(ctx, &msg, &query, resp, is_my_server).await;
            } else if msg.content == "<?" {
                if let Err(e) = msg.reply_ping(&ctx.http, "도움말은 아직 없어").await {
                    eprintln!("[ERROR]  <? reply_ping(): {e:?}");
                }
                return;
            }
        }

        if msg.content.trim_start().starts_with("<!") {
            let pool = er!(get_pool().await, "[RESP] get_pool()");
            handle_rtcmd(ctx, &msg, pool, resp_store).await;
            return;
        }

        if is_my_server {
            if msg.content.starts_with("<&") && msg.author.id == KKITUT_USERID {
                ctx.set_presence(
                    Some(ActivityData::custom("자러가는 중")),
                    OnlineStatus::Idle,
                );
                let embed_off = CreateEmbed::new().title("OFF").color(0x000000);
                let builderoff = CreateMessage::new().embed(embed_off);
                if let Err(e) = CHANNELID_LOG_SECRET
                    .send_message(&ctx.http, builderoff)
                    .await
                {
                    eprintln!("[ERROR] OFF send_message(): {e:?}");
                }
                println!("[SYS] Exiting");
                println!("[SYS] changing stop flag");
                stop_flag.store(true, Ordering::Relaxed);
                println!("[SYS] sending stop notify");
                stop_notify.notify_waiters();
                println!("[SYS] Creating a waiting task");
                let shard = ctx.shard.clone();
                tokio::spawn(async move {
                    println!("[SYS] 10 sec left");
                    sleep(tokio::time::Duration::from_secs(10)).await;
                    println!("[SYS] Shutting down shard");
                    shard.shutdown_clean();
                    println!("[SYS] EXIT");
                    std::process::exit(0);
                });
            } else {
                let channel_id_u64 = msg.channel_id.get();

                let dm_pair = find_dm_either_channel(dm_channels, channel_id_u64).await;

                if let Some((_bot_channel_id, user_channel_id_u64)) = dm_pair {
                    let pool = er!(get_pool_tokidm().await, "[ERROR] DM get_pool_tokidm()");
                    let table_name = format!("c{}u{}", channel_id_u64, user_channel_id_u64);
                    relay_dm_message(
                        ctx,
                        pool,
                        &msg,
                        &table_name,
                        ChannelId::new(user_channel_id_u64),
                        None,
                        false,
                    )
                    .await;
                }
            }
        }
    } else {
        let timestamp = localnow.timestamp();
        let response_str = if let Some(query) = extract_call_query(&msg.content) {
            execute_bot_call(ctx, &msg, &query, resp, false).await
        } else {
            None
        };

        let user_channel_id = msg.channel_id;
        let user_channel_id_u64 = user_channel_id.get();
        let user_id_u64 = msg.author.id.get();

        let table_name_op: Option<String> = find_dm_user_channel(dm_channels, user_channel_id_u64)
            .await
            .map(|(bot_ch_u64, dm_ch_u64)| format!("c{}u{}", bot_ch_u64, dm_ch_u64));

        let pool = er!(get_pool_tokidm().await, "[ERROR] DM get_pool_tokidm()");

        let table_name = if let Some(t_name) = table_name_op {
            t_name
        } else {
            let new_channel = CreateChannel::new(format!("☆▰_dm_{}", user_id_u64))
                .category(ChannelId::new(1348253746798399549))
                .kind(ChannelType::Text)
                .topic(format!(
                    "{} (<@{}>) <t:{}>",
                    msg.author.name, user_id_u64, timestamp
                ));

            let channel_id = match GUILD_ID.create_channel(ctx, new_channel).await {
                Ok(guildchannel) => guildchannel.id,
                Err(e) => {
                    eprintln!(
                        "[ERROR] Failed to create DM log channel {}: {:?}",
                        user_channel_id_u64, e
                    );
                    return;
                }
            };

            let table_name = format!("c{}u{}", channel_id.get(), msg.channel_id.get());

            let mut qb: QueryBuilder<MySql> = QueryBuilder::new("CREATE TABLE IF NOT EXISTS ");
            qb.push(&table_name);
            qb.push(
                " (
                bot BIGINT UNSIGNED NOT NULL,
                user BIGINT UNSIGNED NOT NULL
            )",
            );

            if let Err(e) = qb.build().execute(pool).await {
                eprintln!("[ERROR] Failed to create table {}: {:?}", table_name, e);
            }

            {
                let mut dm_channels_lock = dm_channels.lock().await;
                dm_channels_lock.push((channel_id.get(), user_channel_id_u64));
            }

            table_name
        };

        if let Some(pos) = table_name.find('u') {
            if let Ok(bot_channel_id_u64) = table_name[1..pos].parse::<u64>() {
                relay_dm_message(
                    ctx,
                    pool,
                    &msg,
                    &table_name,
                    ChannelId::new(bot_channel_id_u64),
                    response_str.as_deref(),
                    true,
                )
                .await;
            }
        }
    }
}
