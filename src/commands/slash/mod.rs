pub mod birthday;
pub mod delete;
pub mod inquiry;
pub mod match_cmd;
pub mod misc;
pub mod spy;
pub mod trap;

use serenity::all::{
    CommandInteraction, ComponentInteraction, CreateAllowedMentions, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseMessage, EditMember, GuildId, Timestamp,
    UserId,
};
use serenity::prelude::Context;
use sqlx::MySqlPool;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::constants::KKITUT_USERID;
use crate::response::Resp;
use crate::utils::lng_trs;
use birthday::{handle_birthday, handle_birthday_button};
use delete::{handle_delete_run, handle_delete_select, DeleteMessagesStore};
use inquiry::handle_inquiry;
use match_cmd::handle_match;
use misc::{handle_call, handle_dont_chat, handle_ping};
use spy::{handle_spy_channel, handle_spy_data, handle_spy_profile};
use trap::{handle_trap_list, handle_trap_remove, handle_trap_set, TrapIdsStore};

pub async fn respond_ephemeral_embed(
    ctx: &Context,
    command: &CommandInteraction,
    description: &str,
) {
    if let Err(e) = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(CreateEmbed::new().description(description).color(0xFFAAAA))
                    .allowed_mentions(CreateAllowedMentions::new().empty_users())
                    .ephemeral(true),
            ),
        )
        .await
    {
        eprintln!("[ERROR] Command create_response(): {:?}", e);
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn dispatch_slash_command(
    ctx: &Context,
    command: &CommandInteraction,
    pool: &MySqlPool,
    resp: &Arc<Mutex<Vec<Resp>>>,
    blacklist_guilds: &Arc<Mutex<Vec<u64>>>,
    delete_messages: &DeleteMessagesStore,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
    trap_ids: &TrapIdsStore,
) {
    let lng = command.locale.as_str() == "ko";
    let cmdname = command.data.name.as_str();

    if cmdname.starts_with("z_") && command.user.id != KKITUT_USERID {
        respond_ephemeral_embed(
            ctx,
            command,
            lng_trs(
                lng,
                "Only the person who developed me can handle this command",
                "나를 개발하는 사람만 이 명령어를 다룰 수 있어",
            ),
        )
        .await;
        return;
    }

    match cmdname {
        "spy_data" => handle_spy_data(ctx, command, lng).await,
        "inquiry" => handle_inquiry(ctx, command, lng).await,
        "spy_profile" => handle_spy_profile(ctx, command, lng).await,
        "birthday_set" | "birthday_remove" => handle_birthday(ctx, command, pool, lng).await,
        "ping" => handle_ping(ctx, command).await,
        "delete_f_sel" => {
            handle_delete_select(ctx, command, delete_messages, dm_channels, true, lng).await
        }
        "delete_e_sel" => {
            handle_delete_select(ctx, command, delete_messages, dm_channels, false, lng).await
        }
        "delete_run" => handle_delete_run(ctx, command, delete_messages, lng).await,
        "dont_chat" => handle_dont_chat(ctx, command, pool, blacklist_guilds, lng).await,
        "call" => handle_call(ctx, command, resp, lng).await,
        "spy_channel" => handle_spy_channel(ctx, command, lng).await,
        "z_trap_list" => handle_trap_list(ctx, command, trap_ids).await,
        "z_trap_set" => handle_trap_set(ctx, command, trap_ids, pool, lng).await,
        "z_trap_remove" => handle_trap_remove(ctx, command, trap_ids, pool, lng).await,
        "match" => handle_match(ctx, command, lng).await,
        _ => {
            if let Err(e) = command
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content(lng_trs(
                                lng,
                                "command does not exist",
                                "존재하지 않는 명령어야",
                            ))
                            .ephemeral(true),
                    ),
                )
                .await
            {
                eprintln!("[ERROR] Command create_response(): {:?}", e);
            }
        }
    }
}

pub async fn dispatch_component(ctx: &Context, component: &ComponentInteraction, pool: &MySqlPool) {
    let lng = component.locale.as_str() == "ko";
    let user_id = component.user.id.get();
    let parts: Vec<&str> = component.data.custom_id.as_str().split(':').collect();

    if parts.len() == 2 {
        if let Ok(user_id_original) = parts[1].parse::<u64>() {
            if user_id == user_id_original {
                match parts[0] {
                    "birthday_leap_forward"
                    | "birthday_leap_keep"
                    | "birthday_leap_backward"
                    | "birthday_leap_forward_update"
                    | "birthday_leap_keep_update"
                    | "birthday_leap_backward_update" => {
                        handle_birthday_button(ctx, component, pool, parts[0], lng).await;
                        return;
                    }
                    _ => {}
                }
            }
        }
    } else if component.data.custom_id.as_str() == "timeout_835743162131546132" {
        let until = Timestamp::from_unix_timestamp(
            (chrono::Utc::now() + chrono::Duration::seconds(5)).timestamp(),
        )
        .unwrap();
        let result = GuildId::new(932966507548397619)
            .edit_member(
                &ctx.http,
                UserId::new(835743162131546132),
                EditMember::new().disable_communication_until_datetime(until),
            )
            .await;
        let response_content = match result {
            Ok(_) => "✅",
            Err(e) => {
                eprintln!("[ERROR] Failed timeout_835743162131546132: {:?}", e);
                "❌"
            }
        };
        if let Err(e) = component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(response_content)
                        .ephemeral(true),
                ),
            )
            .await
        {
            eprintln!("[ERROR] Button timeout_835743162131546132: {:?}", e);
        }
        return;
    } else if component.data.custom_id.starts_with('_') {
        let embed = CreateEmbed::new()
            .title(lng_trs(lng, "It's a dummy button", "더미 버튼이야"))
            .color(0xFFAAAA);
        if let Err(e) = component
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .add_embed(embed)
                        .components(vec![])
                        .ephemeral(true),
                ),
            )
            .await
        {
            eprintln!("[ERROR] Unknown error create_response(): {:?}", e);
        }
        return;
    }

    let embed = CreateEmbed::new()
        .title(lng_trs(lng, "Unknown error", "알 수 없는 오류야"))
        .color(0xFF0000);
    if let Err(e) = component
        .create_response(
            &ctx.http,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .add_embed(embed)
                    .components(vec![]),
            ),
        )
        .await
    {
        eprintln!("[ERROR] Unknown error create_response(): {:?}", e);
    }
}
