pub mod deunicode;
pub mod embed;
pub mod memory;
pub mod resp;

use serenity::all::Message;
use serenity::prelude::Context;
use sqlx::MySqlPool;
use std::sync::Arc;

use crate::constants::{DISCORD_ID_ADMINS, RTCMD_HELP_MSG};
use crate::response::RespStore;
use deunicode::handle_deunicode;
use embed::handle_embed;
use memory::handle_memory;
use resp::handle_resp;

pub async fn handle_rtcmd(
    ctx: &Context,
    msg: &Message,
    pool: &MySqlPool,
    resp_store: &Arc<RespStore>,
) {
    if !DISCORD_ID_ADMINS.contains(&msg.author.id.get()) {
        return;
    }

    let rtcmd = msg.content.trim_start_matches("<!").trim_start();

    if rtcmd.is_empty() {
        if let Err(e) = msg.channel_id.say(&ctx.http, RTCMD_HELP_MSG).await {
            eprintln!("[ERROR] <! say(): {:?}", e);
        }
        return;
    }

    if rtcmd.starts_with("memory") {
        handle_memory(ctx, msg).await;
    } else if let Some(rest) = rtcmd.strip_prefix("embed") {
        handle_embed(ctx, msg, rest).await;
    } else if let Some(rest) = rtcmd.strip_prefix("deunicode") {
        handle_deunicode(ctx, msg, rest).await;
    } else if rtcmd.starts_with("resp") {
        handle_resp(ctx, msg, pool, resp_store, rtcmd).await;
    }
}
