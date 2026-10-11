use serenity::all::{
    Channel, ChannelType, CommandInteraction, CreateAllowedMentions, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::prelude::Context;
use sqlx::MySqlPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::respond_ephemeral_embed;
use crate::constants::BOT_USERID;
use crate::utils::lng_trs;

pub type TrapIdsStore = Arc<Mutex<HashMap<u64, (u64, Option<u64>)>>>;

pub async fn handle_trap_list(
    ctx: &Context,
    command: &CommandInteraction,
    trap_ids: &TrapIdsStore,
) {
    let trap_list = { trap_ids.lock().await.clone() };
    let mut list_string = String::new();
    list_string.push_str("```\n");
    list_string.push_str("+------------------+------------------+------------------+\n");
    list_string.push_str("| guild_id         | channel_id       | log_id           |\n");
    list_string.push_str("+------------------+------------------+------------------+\n");

    for (guild_id, (channel_id, log_id)) in trap_list {
        let log_str = match log_id {
            Some(v) => v.to_string(),
            None => "None".to_string(),
        };

        list_string.push_str(&format!(
            "| {:<16} | {:<16} | {:<16} |\n",
            guild_id, channel_id, log_str
        ));
    }

    list_string.push_str("+------------------+------------------+------------------+\n```");

    respond_ephemeral_embed(ctx, command, &list_string).await;
}

pub async fn handle_trap_set(
    ctx: &Context,
    command: &CommandInteraction,
    trap_ids: &TrapIdsStore,
    pool: &MySqlPool,
    lng: bool,
) {
    let respstr;
    if let Some(guild_id) = command.guild_id {
        if let Some(opt0) = command.data.options.first() {
            if let Some(trigger_channel) = opt0.value.as_channel_id() {
                match ctx.http.get_channel(trigger_channel).await {
                    Ok(tri_ch) => {
                        match tri_ch {
                            Channel::Guild(ch) => {
                                if ch.guild_id == guild_id {
                                    if ch.kind == ChannelType::Text {
                                        let log_channel =
                                            if let Some(opt1) = command.data.options.get(1) {
                                                if let Some(log_ch) = opt1.value.as_channel_id() {
                                                    Some(log_ch)
                                                } else {
                                                    respond_ephemeral_embed(
                                                        ctx,
                                                        command,
                                                        lng_trs(
                                                            lng,
                                                            "Log channel is invalid",
                                                            "로그 채널이 잘못되었어",
                                                        ),
                                                    )
                                                    .await;
                                                    return;
                                                }
                                            } else {
                                                None
                                            };

                                        if let Some(bot_perms) = command.app_permissions {
                                            if bot_perms.ban_members() {
                                                if let Some(log_ch) = log_channel {
                                                    let can_send_log = match ctx.http.get_channel(log_ch).await {
                                                    Ok(log_ch) => match log_ch {
                                                        Channel::Guild(ch) => {
                                                            if ch.guild_id == guild_id {
                                                                if ch.kind == ChannelType::Text {
                                                                    match ctx.http.get_guild(guild_id).await {
                                                                        Ok(guild) => {
                                                                            match ctx.http.get_member(guild_id, BOT_USERID).await {
                                                                                Ok(bot) => {
                                                                                    if guild.user_permissions_in(&ch, &bot).send_messages() {
                                                                                        None
                                                                                    } else {
                                                                                        Some(lng_trs(
                                                                                            lng,
                                                                                            "I don't have permission to send messages from the log server",
                                                                                            "로그 서버에서 메시지를 전송할 권한이 없어",
                                                                                        ))
                                                                                    }
                                                                                }
                                                                                Err(e) => {
                                                                                    eprintln!("[ERROR] Command get_member(): {:?}", e);
                                                                                    Some(lng_trs(
                                                                                        lng,
                                                                                        "Failed to retrieve member information to view my permissions",
                                                                                        "내 권한을 보기 위한 맴버 정보를 불러오는데 실패했어",
                                                                                    ))
                                                                                }
                                                                            }
                                                                        }
                                                                        Err(e) => {
                                                                            eprintln!("[ERROR] Command get_guild(): {:?}", e);
                                                                            Some(lng_trs(
                                                                                lng,
                                                                                "Failed to retrieve server information to view my permissions",
                                                                                "내 권한을 보기 위한 서버 정보를 불러오는데 실패했어",
                                                                            ))
                                                                        }
                                                                    }
                                                                } else {
                                                                    Some(lng_trs(
                                                                        lng,
                                                                        "Log channel type is not a text channel",
                                                                        "로그 체널의 타입이 텍스트 체널이 아니야",
                                                                    ))
                                                                }
                                                            } else {
                                                                Some(lng_trs(
                                                                    lng,
                                                                    "Log channel is not a channel of this server",
                                                                    "로그 채널이 이 서버의 체널이 아니야",
                                                                ))
                                                            }
                                                        }
                                                        _ => Some(lng_trs(
                                                            lng,
                                                            "Log channel is not a server channel",
                                                            "로그 채널이 서버 체널이 아니야",
                                                        )),
                                                    },
                                                    Err(e) => {
                                                        eprintln!("[ERROR] Command get_channel(): {:?}", e);
                                                        Some(lng_trs(
                                                            lng,
                                                            "Log channel is missing",
                                                            "로그 채널을 찾을 수 없어",
                                                        ))
                                                    }
                                                };
                                                    if let Some(log) = can_send_log {
                                                        if let Err(e) = command
                                                        .create_response(
                                                            &ctx.http,
                                                            CreateInteractionResponse::Message(
                                                                CreateInteractionResponseMessage::new()
                                                                    .embed(
                                                                        CreateEmbed::new()
                                                                            .description(log)
                                                                            .color(0xFFAAAA),
                                                                    )
                                                                    .allowed_mentions(
                                                                        CreateAllowedMentions::new().empty_users(),
                                                                    )
                                                                    .ephemeral(true),
                                                            ),
                                                        )
                                                        .await
                                                    {
                                                        eprintln!("[ERROR] Command create_response(): {:?}", e);
                                                    }
                                                        return;
                                                    }
                                                }

                                                let exists = {
                                                    let lock = trap_ids.lock().await;
                                                    lock.contains_key(&guild_id.get())
                                                };

                                                if !exists {
                                                    let trap_info = (
                                                        guild_id.get(),
                                                        trigger_channel.get(),
                                                        log_channel.map(|c| c.get()),
                                                    );
                                                    match sqlx::query(
                                                    "INSERT INTO trap (guild_id, channel_id, log_id) VALUES (?, ?, ?)",
                                                )
                                                .bind(trap_info.0)
                                                .bind(trap_info.1)
                                                .bind(trap_info.2)
                                                .execute(pool)
                                                .await
                                                {
                                                    Ok(_) => {
                                                        {
                                                            let mut lock = trap_ids.lock().await;
                                                            lock.insert(
                                                                trap_info.0,
                                                                (trap_info.1, trap_info.2),
                                                            );
                                                        }
                                                        respstr = lng_trs(
                                                            lng,
                                                            "Trap has been set",
                                                            "함정이 설정되었어",
                                                        );
                                                    }
                                                    Err(e) => {
                                                        eprintln!("[ERROR] Command sqlx::query(): {:?}", e);
                                                        respstr = lng_trs(
                                                            lng,
                                                            "An error occurred while saving to the database",
                                                            "DB에 저장하던 중에 오류가 발생했어",
                                                        );
                                                    }
                                                }
                                                } else {
                                                    respstr = lng_trs(
                                                        lng,
                                                        "Trap has already been set",
                                                        "함정이 이미 설정되어있어",
                                                    );
                                                }
                                            } else {
                                                respstr = lng_trs(
                                                    lng,
                                                    "I do not have permission to ban members",
                                                    "유저를 밴할 수 있는 권한이 없어",
                                                );
                                            }
                                        } else {
                                            respstr = lng_trs(
                                                lng,
                                                "Unable to check bot permissions",
                                                "내 권한을 확인할 수 없었어",
                                            );
                                        }
                                    } else {
                                        respstr = lng_trs(
                                            lng,
                                            "Trigger channel type is not a text channel",
                                            "발동 체널의 타입이 텍스트 체널이 아니야",
                                        );
                                    }
                                } else {
                                    respstr = lng_trs(
                                        lng,
                                        "Trigger channel is not a channel of this server",
                                        "발동 채널이 이 서버의 체널이 아니야",
                                    );
                                }
                            }
                            _ => {
                                respstr = lng_trs(
                                    lng,
                                    "Trigger channel is not a server channel",
                                    "발동 채널이 서버 체널이 아니야",
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Command get_channel(): {:?}", e);
                        respstr = lng_trs(
                            lng,
                            "Trigger channel is missing",
                            "발동 채널을 찾을 수 없어",
                        );
                    }
                }
            } else {
                respstr = lng_trs(lng, "Trigger channel is invalid", "발동 채널이 잘못되었어");
            }
        } else {
            respstr = lng_trs(lng, "Trigger channel is empty", "발동 채널이 비었어");
        }
    } else {
        respstr = lng_trs(lng, "There isn't a guild", "여긴 서버가 아니야");
    }

    if let Err(e) = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(CreateEmbed::new().description(respstr).color(0xFFAAAA))
                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                    .ephemeral(true),
            ),
        )
        .await
    {
        eprintln!("[ERROR] Command create_response(): {:?}", e);
    }
}

pub async fn handle_trap_remove(
    ctx: &Context,
    command: &CommandInteraction,
    trap_ids: &TrapIdsStore,
    pool: &MySqlPool,
    lng: bool,
) {
    let respstr;
    if let Some(guild_id) = command.guild_id {
        let guild_id_raw = guild_id.get();
        let exists = {
            let lock = trap_ids.lock().await;
            lock.contains_key(&guild_id_raw)
        };

        if exists {
            match sqlx::query("DELETE FROM trap WHERE guild_id = ?")
                .bind(guild_id_raw)
                .execute(pool)
                .await
            {
                Ok(_) => {
                    {
                        let mut lock = trap_ids.lock().await;
                        lock.remove(&guild_id_raw);
                    }
                    respstr = lng_trs(lng, "Trap has been deleted", "함정이 삭제되었어");
                }
                Err(e) => {
                    eprintln!("[ERROR] Command sqlx::query(): {:?}", e);
                    respstr = lng_trs(
                        lng,
                        "An error occurred while deleting from the DB",
                        "DB에서 삭제하던 중에 오류가 발생했어",
                    );
                }
            }
        } else {
            respstr = lng_trs(lng, "No trap set", "함정이 설정되어 있지 않아");
        }
    } else {
        respstr = lng_trs(lng, "There isn't a guild", "여긴 서버가 아니야");
    }

    respond_ephemeral_embed(ctx, command, respstr).await;
}
