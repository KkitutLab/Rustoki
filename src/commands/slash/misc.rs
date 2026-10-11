use chrono::Utc;
use serenity::all::{
    CommandInteraction, CreateAllowedMentions, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage, Permissions,
};
use serenity::prelude::Context;
use sqlx::MySqlPool;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::respond_ephemeral_embed;
use crate::response::{get_response, get_special_response, Resp};
use crate::utils::lng_trs;

pub async fn handle_ping(ctx: &Context, command: &CommandInteraction) {
    let ts = command.id.created_at();
    let elapsed_ms = Utc::now().timestamp_millis() - ts.timestamp_millis();

    if let Err(e) = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(
                        CreateEmbed::new()
                            .title(format!("{}ms", elapsed_ms))
                            .color(0xFF8888),
                    )
                    .allowed_mentions(CreateAllowedMentions::new().empty_users()),
            ),
        )
        .await
    {
        eprintln!("[ERROR] Command create_response(): {:?}", e);
    }
}

pub async fn handle_dont_chat(
    ctx: &Context,
    command: &CommandInteraction,
    pool: &MySqlPool,
    blacklist_guilds: &Arc<Mutex<Vec<u64>>>,
    lng: bool,
) {
    let respstr = if let Some(guild_id) = command.guild_id {
        if let Some(ref member) = command.member {
            if member
                .permissions
                .unwrap_or(Permissions::empty())
                .administrator()
            {
                if let Some(opt) = command.data.options.first() {
                    if let Some(set) = opt.value.as_bool() {
                        if set {
                            match sqlx::query("DELETE FROM message_blacklist_guild WHERE id = ?")
                                .bind(guild_id.get())
                                .execute(pool)
                                .await
                            {
                                Ok(_) => {
                                    {
                                        let mut guard = blacklist_guilds.lock().await;
                                        guard.retain(|&id| id != guild_id.get());
                                    }
                                    lng_trs(lng, "I can now chat", "이제 대화할 수 있어")
                                }
                                Err(_) => lng_trs(
                                    lng,
                                    "Failed to allow chatting due to DB error",
                                    "대화 허용을 위한 DB 접근에 실패했어",
                                ),
                            }
                        } else {
                            match sqlx::query(
                                "INSERT IGNORE INTO message_blacklist_guild (id) VALUES (?)",
                            )
                            .bind(guild_id.get())
                            .execute(pool)
                            .await
                            {
                                Ok(_) => {
                                    {
                                        let mut guard = blacklist_guilds.lock().await;
                                        guard.push(guild_id.get());
                                    }
                                    lng_trs(lng, "I can no longer chat", "이제 대화할 수 없어")
                                }
                                Err(_) => lng_trs(
                                    lng,
                                    "Failed to block chatting due to DB error",
                                    "대화 차단을 위한 DB 접근에 실패했어",
                                ),
                            }
                        }
                    } else {
                        lng_trs(lng, "Option value invalid", "옵션 값이 잘못되었어")
                    }
                } else {
                    lng_trs(lng, "Option value missing", "옵션 값이 없어")
                }
            } else {
                lng_trs(lng, "You are not an admin.", "넌 관리자가 아니야")
            }
        } else {
            lng_trs(
                lng,
                "Unable to get Your member info",
                "너의 맴버 정보를 가져올 수 없었어",
            )
        }
    } else {
        lng_trs(lng, "There isn't a guild", "여긴 서버가 아니야")
    };

    respond_ephemeral_embed(ctx, command, respstr).await;
}

pub async fn handle_call(
    ctx: &Context,
    command: &CommandInteraction,
    resp: &Arc<Mutex<Vec<Resp>>>,
    lng: bool,
) {
    let respstr;
    if let Some(opt) = command.data.options.first() {
        if let Some(message) = opt.value.as_str() {
            if message == "나" || message == "ME" {
                if let Some(rsp) =
                    get_response(resp, &format!("<@{}>", command.user.id.get())).await
                {
                    respstr = rsp;
                } else {
                    respstr = format!("{}{}", command.user.name, "은/는 DB에 없어");
                }
            } else if let Some(rsp) = get_response(resp, message).await {
                respstr = rsp;
            } else if let Some(spmsg) = get_special_response(message, ctx).await {
                if spmsg.extra_note.starts_with("*sticker") {
                    respstr = "(여기선 스티커를 보낼 수가 없어)".to_string();
                } else {
                    if let Err(e) = command
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(spmsg.to_interaction_response()),
                        )
                        .await
                    {
                        eprintln!("[ERROR] Command create_response(): {:?}", e);
                    }
                    return;
                }
            } else {
                respstr = "?".to_string();
            }
        } else {
            respstr =
                lng_trs(lng, "Answer string is invalid", "질문 문자열에 오류가 있어").to_string();
        }
    } else {
        respstr = lng_trs(lng, "Your answer is empty", "답변이 비었어").to_string();
    }

    if let Err(e) = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(
                        CreateEmbed::new()
                            .title(if respstr == "?" { "?" } else { "" })
                            .description(if respstr == "?" { "" } else { &respstr })
                            .color(0xFFAAAA),
                    )
                    .allowed_mentions(CreateAllowedMentions::new().empty_users()),
            ),
        )
        .await
    {
        eprintln!("[ERROR] Command create_response(): {:?}", e);
    }
}
