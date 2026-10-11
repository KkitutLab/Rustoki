use serenity::all::{
    ActionRowComponent, ChannelId, CreateActionRow, CreateAllowedMentions, CreateAttachment,
    CreateButton, CreateEmbed, CreateMessage, Message, MessageId, MessageReference,
    MessageReferenceKind, StickerType,
};
use serenity::prelude::Context;
use sqlx::{MySql, MySqlPool, QueryBuilder};

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::constants::BOT_USERID;
use crate::db::models::MessagePair;

pub async fn find_dm_bot_channel(
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
    bot_ch: u64,
) -> Option<(u64, u64)> {
    let guard = dm_channels.lock().await;
    guard.iter().find(|&&(b, _)| b == bot_ch).copied()
}

pub async fn find_dm_user_channel(
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
    user_ch: u64,
) -> Option<(u64, u64)> {
    let guard = dm_channels.lock().await;
    guard.iter().find(|&&(_, u)| u == user_ch).copied()
}

pub async fn find_dm_either_channel(
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
    ch: u64,
) -> Option<(u64, u64)> {
    let guard = dm_channels.lock().await;
    guard.iter().find(|&&(b, u)| b == ch || u == ch).copied()
}

pub async fn find_message_pair(
    pool: &MySqlPool,
    table_name: &str,
    message_id: u64,
) -> Result<Option<MessagePair>, sqlx::Error> {
    let mut qb: QueryBuilder<MySql> = QueryBuilder::new("SELECT bot, user FROM ");
    qb.push(table_name);
    qb.push(" WHERE user = ");
    qb.push_bind(message_id);

    let result = qb
        .build_query_as::<MessagePair>()
        .fetch_optional(pool)
        .await?;

    if result.is_some() {
        return Ok(result);
    }

    let mut qb: QueryBuilder<MySql> = QueryBuilder::new("SELECT bot, user FROM ");
    qb.push(table_name);
    qb.push(" WHERE bot = ");
    qb.push_bind(message_id);

    qb.build_query_as::<MessagePair>()
        .fetch_optional(pool)
        .await
}

pub async fn build_relay_message(
    ctx: &Context,
    pool: &MySqlPool,
    msg: &Message,
    table_name: &str,
    target_channel_id: ChannelId,
    sys_response_prefix: Option<&str>,
) -> (CreateMessage, bool) {
    let mut message = CreateMessage::new();
    let mut is_not_empty = false;

    if !msg.content.is_empty() {
        if let Some(resp_prefix) = sys_response_prefix {
            message = message.embed(
                CreateEmbed::new()
                    .title(format!("[SYS] >> {}", resp_prefix))
                    .description(&msg.content)
                    .color(0xFFAAAA),
            );
        } else {
            message = message.content(msg.content.replace('@', "＠"));
        }
        is_not_empty = true;
    }

    let mut attachments = Vec::new();
    for attachment in &msg.attachments {
        if let Ok(create_attachment) = CreateAttachment::url(&ctx.http, &attachment.url).await {
            attachments.push(create_attachment);
        }
    }
    if !attachments.is_empty() {
        message = message.add_files(attachments);
        is_not_empty = true;
    }

    let mut sticker_ids = Vec::new();
    for sticker_item in &msg.sticker_items {
        if let Ok(sticker) = sticker_item.id.to_sticker(&ctx.http).await {
            if sticker.kind != StickerType::Guild {
                sticker_ids.push(sticker.id);
            }
        }
    }
    if !sticker_ids.is_empty() {
        message = message.add_sticker_ids(sticker_ids);
        is_not_empty = true;
    }

    if let Some(message_reference) = &msg.message_reference {
        if let Some(ref_id) = message_reference.message_id {
            if let Ok(Some(row)) = find_message_pair(pool, table_name, ref_id.get()).await {
                let target_ref_id = row.target_for(ref_id.get());

                let reference = MessageReference::new(message_reference.kind, target_channel_id)
                    .message_id(MessageId::new(target_ref_id));
                message = message.reference_message(reference);

                if !msg
                    .mentions
                    .iter()
                    .any(|m| m.id == BOT_USERID || m.id == msg.author.id)
                {
                    message = message.allowed_mentions(CreateAllowedMentions::new().empty_users());
                }
            }
        }
    }

    if !msg.embeds.is_empty() {
        let mut sendembeds = Vec::new();
        for embed in &msg.embeds {
            sendembeds.push(embed.clone().into());
        }
        message = message.embeds(sendembeds);
        is_not_empty = true;
    }

    if !msg.components.is_empty() {
        let mut pid = 0;
        let mut sendcomponents = Vec::new();

        for actionrow in &msg.components {
            let mut buttons = Vec::new();

            for component in &actionrow.components {
                if let ActionRowComponent::Button(button) = component {
                    let mut button_block: CreateButton = button.clone().into();
                    button_block = button_block.custom_id(format!("_{}", pid));
                    pid += 1;
                    buttons.push(button_block);
                }
            }

            if !buttons.is_empty() {
                sendcomponents.push(CreateActionRow::Buttons(buttons));
                is_not_empty = true;
            }
        }

        message = message.components(sendcomponents);
    }

    if !is_not_empty && msg.interaction_metadata.is_some() {
        message = message.content("상대방이 사용한 앱의 응답을 기다리고 있어");
        is_not_empty = true;
    }

    (message, is_not_empty)
}

pub async fn relay_dm_message(
    ctx: &Context,
    pool: &MySqlPool,
    msg: &Message,
    table_name: &str,
    target_channel_id: ChannelId,
    sys_response_prefix: Option<&str>,
    is_dm_to_bot: bool,
) {
    let (message, is_not_empty) = build_relay_message(
        ctx,
        pool,
        msg,
        table_name,
        target_channel_id,
        sys_response_prefix,
    )
    .await;

    let is_err = if is_not_empty {
        match target_channel_id.send_message(&ctx.http, message).await {
            Ok(sent_message) => {
                let (bot_id, user_id) = (sent_message.id.get(), msg.id.get());

                let mut qb: QueryBuilder<MySql> = QueryBuilder::new("INSERT INTO ");
                qb.push(table_name);
                qb.push(" (bot, user) VALUES (");

                let mut separated = qb.separated(", ");
                separated.push_bind(bot_id);
                separated.push_bind(user_id);

                qb.push(")");

                if let Err(e) = qb.build().execute(pool).await {
                    eprintln!("[ERROR] Failed to insert into {}: {:?}", table_name, e);
                }

                0
            }
            Err(e) => {
                eprintln!(
                    "[ERROR] Failed to send DM to channel {}: {:?}",
                    target_channel_id.get(),
                    e
                );
                2
            }
        }
    } else {
        1
    };

    if is_err != 0 {
        let title = if is_err == 1 {
            "Unsendable message"
        } else {
            "An internal error occurred"
        };
        let embed = CreateEmbed::new().title(title).color(0x2B001E);

        if let Err(e) = msg
            .channel_id
            .send_message(
                &ctx.http,
                CreateMessage::new().embed(embed).reference_message(
                    MessageReference::new(MessageReferenceKind::Default, msg.channel_id)
                        .message_id(msg.id),
                ),
            )
            .await
        {
            eprintln!(
                "[ERROR] Failed to send info at DM channel user {}: {:?}",
                msg.author.id.get(),
                e
            );
        }

        if is_dm_to_bot {
            if let Err(e) = target_channel_id
                .send_message(
                    &ctx.http,
                    CreateMessage::new().embed(
                        CreateEmbed::new()
                            .description(format!("-# *[SYS] >> {}: {} *", title, msg.id.get()))
                            .color(0x2B001E),
                    ),
                )
                .await
            {
                eprintln!(
                    "[ERROR] Failed to send DM err to channel {}: {:?}",
                    msg.id.get(),
                    e
                );
            }
        }
    }
}
