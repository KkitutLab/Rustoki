use chrono::Local;
use serenity::all::{
    ChannelId, CommandInteraction, CreateAllowedMentions, CreateEmbed, EditInteractionResponse,
    GetMessages, MessageId, Permissions,
};
use serenity::prelude::Context;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::constants::GUILD_ID;
use crate::utils::lng_trs;

pub type DeleteMessagesStore = Arc<
    Mutex<
        Vec<(
            u64,
            (u64, u64, u64),
            (u64, u64, u64),
            chrono::DateTime<Local>,
        )>,
    >,
>;

pub async fn handle_delete_select(
    ctx: &Context,
    command: &CommandInteraction,
    delete_messages: &DeleteMessagesStore,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
    is_first: bool,
    lng: bool,
) {
    if let Err(e) = command.defer_ephemeral(&ctx.http).await {
        eprintln!("[ERROR] Command defer(): {:?}", e);
        return;
    }

    let respstr;
    let user_id = command.user.id.get();

    if let Some(message_id) = command.data.target_id {
        if let Some(guild_id) = command.guild_id {
            if let Some(ref member) = command.member {
                let perms = member.permissions.unwrap_or(Permissions::empty());
                if perms.manage_messages() {
                    let channel_id = command.channel_id;

                    let in_admin_dm = guild_id.get() == GUILD_ID.get() && {
                        let guard = dm_channels.lock().await;
                        guard.iter().any(|&(ch, _)| ch == channel_id.get())
                    };

                    if !in_admin_dm {
                        let id = message_id.get();
                        let mut duplicate = false;

                        {
                            let mut lock = delete_messages.lock().await;

                            if let Some(entry) =
                                lock.iter_mut().find(|(uid, _, _, _)| *uid == user_id)
                            {
                                let other_id = if is_first { entry.2 .0 } else { entry.1 .0 };
                                if other_id == id {
                                    duplicate = true;
                                } else {
                                    let slot = if is_first { &mut entry.1 } else { &mut entry.2 };
                                    slot.0 = id;
                                    slot.1 = guild_id.get();
                                    slot.2 = channel_id.get();
                                    entry.3 = Local::now();
                                }
                            } else {
                                let current_target = (id, guild_id.get(), channel_id.get());
                                let empty_target = (0, 0, 0);
                                if is_first {
                                    lock.push((
                                        user_id,
                                        current_target,
                                        empty_target,
                                        Local::now(),
                                    ));
                                } else {
                                    lock.push((
                                        user_id,
                                        empty_target,
                                        current_target,
                                        Local::now(),
                                    ));
                                }
                            }
                        }

                        if duplicate {
                            respstr =
                                lng_trs(lng, "Value is duplicated", "값이 중복이야").to_string();
                        } else {
                            respstr = format!(
                                "{}\nID: {}",
                                if is_first {
                                    lng_trs(lng, "First Message Selected", "첫 메시지를 선택했어")
                                } else {
                                    lng_trs(lng, "End Message Selected", "마지막 메시지를 선택했어")
                                },
                                message_id.to_message_id().link(channel_id, Some(guild_id))
                            );
                        }
                    } else {
                        respstr = lng_trs(
                            lng,
                            "Cannot use in admin DM channel",
                            "관리자 DM 채널에선 사용할 수 없어",
                        )
                        .to_string();
                    }
                } else {
                    respstr = lng_trs(
                        lng,
                        "You do not have permission to manage messages",
                        "넌 메시지 관리 권한이 없어",
                    )
                    .to_string();
                }
            } else {
                respstr = lng_trs(
                    lng,
                    "Unable to get Your member info",
                    "너의 맴버 정보를 가져올 수 없었어",
                )
                .to_string();
            }
        } else {
            respstr =
                lng_trs(lng, "Only available in guilds", "길드에서만 사용 가능해").to_string();
        }
    } else {
        respstr = lng_trs(
            lng,
            "Unable to get message ID",
            "메시지 ID를 가져올 수 없었어",
        )
        .to_string();
    }

    edit_ephemeral_embed(ctx, command, &respstr).await;
}

pub async fn edit_ephemeral_embed(ctx: &Context, command: &CommandInteraction, description: &str) {
    if let Err(e) = command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(CreateEmbed::new().description(description).color(0xFFAAAA))
                .allowed_mentions(CreateAllowedMentions::new().empty_users()),
        )
        .await
    {
        eprintln!("[ERROR] Command edit_response(): {:?}", e);
    }
}

pub async fn handle_delete_run(
    ctx: &Context,
    command: &CommandInteraction,
    delete_messages: &DeleteMessagesStore,
    lng: bool,
) {
    if let Err(e) = command.defer_ephemeral(&ctx.http).await {
        eprintln!("[ERROR] Command defer(): {:?}", e);
        return;
    }

    let respstr;

    if let Some(ref member) = command.member {
        let perms = member.permissions.unwrap_or(Permissions::empty());
        if perms.manage_messages() {
            if let Some(bot_perms) = command.app_permissions {
                if bot_perms.manage_messages() {
                    let user_id = command.user.id.get();
                    let entry_opt = {
                        let lock = delete_messages.lock().await;
                        lock.iter().find(|(uid, _, _, _)| *uid == user_id).cloned()
                    };

                    if let Some(entry) = entry_opt {
                        if entry.1 .0 == 0 {
                            respstr = lng_trs(
                                lng,
                                "First message not selected",
                                "첫 메시지가 선택되지 않았어",
                            )
                            .to_string();
                        } else if entry.2 .0 == 0 {
                            respstr = lng_trs(
                                lng,
                                "End message not selected",
                                "끝 메시지가 선택되지 않았어",
                            )
                            .to_string();
                        } else if entry.1 .1 != entry.2 .1 {
                            respstr = lng_trs(
                                lng,
                                "Each message has a different guild",
                                "각 메시지의 길드가 달라",
                            )
                            .to_string();
                        } else if entry.1 .2 != entry.2 .2 {
                            respstr = lng_trs(
                                lng,
                                "Each message has a different channel",
                                "각 메시지의 채널이 달라",
                            )
                            .to_string();
                        } else {
                            let (start_id, end_id) = if entry.1 .0 < entry.2 .0 {
                                (entry.1 .0, entry.2 .0)
                            } else {
                                (entry.2 .0, entry.1 .0)
                            };
                            let channel_id = ChannelId::new(entry.1 .2);
                            let max_loops = 20;
                            let mut last_id = start_id;
                            let mut messages_to_delete: Vec<MessageId> = Vec::new();
                            let mut failed_error = None;

                            match channel_id.message(&ctx.http, start_id).await {
                                Ok(msg) => messages_to_delete.push(msg.id),
                                Err(e) => failed_error = Some(e),
                            }

                            let mut reached_max_loops = false;
                            if failed_error.is_none() {
                                for i in 0..max_loops {
                                    let messages = match channel_id
                                        .messages(
                                            &ctx.http,
                                            GetMessages::new().after(last_id).limit(50),
                                        )
                                        .await
                                    {
                                        Ok(msgs) => msgs,
                                        Err(e) => {
                                            failed_error = Some(e);
                                            break;
                                        }
                                    };

                                    if messages.is_empty() {
                                        break;
                                    }

                                    for message in &messages {
                                        let message_id = message.id.get();
                                        if message_id <= end_id {
                                            messages_to_delete.push(message.id);
                                        }
                                        if message_id > last_id {
                                            last_id = message_id;
                                        }
                                    }

                                    if last_id >= end_id {
                                        break;
                                    }

                                    if i + 1 == max_loops {
                                        reached_max_loops = true;
                                    }
                                }
                            }

                            if let Some(e) = failed_error {
                                respstr = format!(
                                    "{}: {}",
                                    lng_trs(
                                        lng,
                                        "Failed to fetch messages",
                                        "메시지 가져오기에 실패했어"
                                    ),
                                    e
                                );
                            } else if messages_to_delete.is_empty() {
                                if reached_max_loops {
                                    respstr = lng_trs(
                                        lng,
                                        "Too many messages to delete. (over) Please select a smaller range",
                                        "삭제할 메시지가 너무 많아. (1000 초과) 더 작은 범위를 선택해줘",
                                    )
                                    .to_string();
                                } else {
                                    respstr = lng_trs(
                                        lng,
                                        "No messages to delete",
                                        "삭제할 메시지가 없어",
                                    )
                                    .to_string();
                                }
                            } else {
                                let mut delete_error = None;
                                for chunk in messages_to_delete.chunks(50) {
                                    if let Err(e) =
                                        channel_id.delete_messages(&ctx.http, chunk.iter()).await
                                    {
                                        delete_error = Some(format!(
                                            "{}: {}",
                                            lng_trs(
                                                lng,
                                                "Failed to delete messages",
                                                "메시지 삭제에 실패했어"
                                            ),
                                            e
                                        ));
                                        break;
                                    }
                                }

                                if let Some(e) = delete_error {
                                    respstr = e;
                                } else {
                                    respstr = format!(
                                        "{} {}",
                                        messages_to_delete.len(),
                                        lng_trs(
                                            lng,
                                            "messages deleted",
                                            "개의 메시지가 삭제되었어"
                                        )
                                    );
                                    let mut lock = delete_messages.lock().await;
                                    lock.retain(|(uid, _, _, _)| *uid != user_id);
                                }
                            }
                        }
                    } else {
                        respstr = lng_trs(lng, "Not selected", "선택되지 않았어").to_string();
                    }
                } else {
                    respstr = lng_trs(
                        lng,
                        "I do not have permission to manage messages",
                        "메시지 관리 권한이 없어",
                    )
                    .to_string();
                }
            } else {
                respstr = lng_trs(
                    lng,
                    "Unable to check bot permissions",
                    "내 권한을 확인할 수 없었어",
                )
                .to_string();
            }
        } else {
            respstr = lng_trs(
                lng,
                "You do not have permission to manage messages",
                "넌 메시지 관리 권한이 없어",
            )
            .to_string();
        }
    } else {
        respstr = lng_trs(
            lng,
            "Unable to get Your member info",
            "너의 맴버 정보를 가져올 수 없었어",
        )
        .to_string();
    }

    edit_ephemeral_embed(ctx, command, &respstr).await;
}
