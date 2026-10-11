use serenity::all::{
    ActionRowComponent, ChannelId, CreateActionRow, CreateAttachment, CreateButton, CreateEmbed,
    CreateMessage, EditAttachments, EditMessage, GuildId, MessageId, MessageReference,
    MessageReferenceKind, MessageUpdateEvent,
};
use serenity::prelude::Context;
use sqlx::{MySql, QueryBuilder};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::constants::{BOT_USERID, GUILD_ID};
use crate::db::get_pool_tokidm;
use crate::dm::find_dm_either_channel;
use crate::macros::er;

pub async fn handle_message_delete(
    ctx: &Context,
    channel_id: ChannelId,
    message_id: MessageId,
    guild_id: Option<GuildId>,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
) {
    let channel_id_u64 = channel_id.get();

    let dm_pair = find_dm_either_channel(dm_channels, channel_id_u64).await;

    if let Some((bot_channel_id, user_channel_id)) = dm_pair {
        let pool = er!(
            get_pool_tokidm().await,
            "[ERROR] DM message_delete get_pool_tokidm()"
        );
        let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

        if let Some(guild_id) = guild_id {
            if guild_id.get() != GUILD_ID.get() {
                return;
            }

            let mut qb: QueryBuilder<MySql> = QueryBuilder::new("SELECT bot FROM ");
            qb.push(&table_name);
            qb.push(" WHERE user = ");
            qb.push_bind(message_id.get());

            if let Ok(bot_message_id) = qb.build_query_scalar::<u64>().fetch_one(pool).await {
                if let Err(e) = ChannelId::new(user_channel_id)
                    .delete_message(&ctx.http, MessageId::new(bot_message_id))
                    .await
                {
                    eprintln!(
                        "[ERROR] Failed to delete DM message {}: {}",
                        user_channel_id, e
                    );
                } else {
                    let mut qb: QueryBuilder<MySql> = QueryBuilder::new("DELETE FROM ");
                    qb.push(&table_name);
                    qb.push(" WHERE bot = ");
                    qb.push_bind(bot_message_id);
                    let _ = qb.build().execute(pool).await;
                }
            }
            return;
        }

        let mut qb_select: QueryBuilder<MySql> = QueryBuilder::new("SELECT bot FROM ");
        qb_select.push(&table_name);
        qb_select.push(" WHERE user = ");
        qb_select.push_bind(message_id.get());

        if let Ok(bot_message_id) = qb_select.build_query_scalar::<u64>().fetch_one(pool).await {
            let mut qb_delete: QueryBuilder<MySql> = QueryBuilder::new("DELETE FROM ");
            qb_delete.push(&table_name);
            qb_delete.push(" WHERE bot = ");
            qb_delete.push_bind(bot_message_id);

            let _ = qb_delete.build().execute(pool).await;

            let embed = CreateEmbed::new().title("제거됨").color(0x2B001E);

            let bot_channel = ChannelId::new(bot_channel_id);

            let _ = bot_channel
                .send_message(
                    &ctx.http,
                    CreateMessage::new().embed(embed).reference_message(
                        MessageReference::new(MessageReferenceKind::Default, bot_channel)
                            .message_id(MessageId::new(bot_message_id)),
                    ),
                )
                .await;
        }
    }
}

async fn build_edit_message(ctx: &Context, event: &MessageUpdateEvent) -> EditMessage {
    let mut message = EditMessage::new();

    if let Some(content) = &event.content {
        message = message.content(content);
    }

    if let Some(attachments) = &event.attachments {
        let mut attachmentsi = EditAttachments::new();
        for attachment in attachments {
            if let Ok(create_attachment) = CreateAttachment::url(&ctx.http, &attachment.url).await {
                attachmentsi = attachmentsi.add(create_attachment);
            }
        }
        message = message.attachments(attachmentsi);
    }
    if let Some(embeds) = &event.embeds {
        if !embeds.is_empty() {
            let mut sendembeds = Vec::new();
            for embed in embeds {
                sendembeds.push(embed.clone().into());
            }
            message = message.embeds(sendembeds);
        }
    }
    if let Some(components) = &event.components {
        if !components.is_empty() {
            let mut pid = 0;
            let mut sendcomponents = Vec::new();

            for actionrow in components {
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
                }
            }
            message = message.components(sendcomponents);
        }
    }

    message
}

pub async fn handle_message_update(
    ctx: &Context,
    event: MessageUpdateEvent,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
) {
    if let Some(author) = &event.author {
        if author.id == BOT_USERID {
            return;
        }
    }

    let channel_id_u64 = event.channel_id.get();

    if let Some(guild_id) = event.guild_id {
        if guild_id.get() != GUILD_ID.get() {
            return;
        }

        let dm_pair = find_dm_either_channel(dm_channels, channel_id_u64).await;

        if let Some((bot_channel_id, user_channel_id)) = dm_pair {
            let pool = er!(
                get_pool_tokidm().await,
                "[ERROR] DM message_update bot->user get_pool_tokidm()"
            );
            let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

            if let Some(pos) = table_name.find('u') {
                if let Ok(user_channel_id_u64) = table_name[pos + 1..].parse::<u64>() {
                    let mut qb: QueryBuilder<MySql> = QueryBuilder::new("SELECT bot FROM ");
                    qb.push(&table_name);
                    qb.push(" WHERE user = ");
                    qb.push_bind(event.id.get());

                    let result = qb.build_query_scalar::<u64>().fetch_one(pool).await;

                    if let Ok(bot_message_id) = result {
                        let message = build_edit_message(ctx, &event).await;

                        if let Err(e) = ChannelId::new(user_channel_id_u64)
                            .edit_message(&ctx.http, MessageId::new(bot_message_id), message)
                            .await
                        {
                            eprintln!(
                                "[ERROR] Failed to send DM to user {}: {}",
                                user_channel_id_u64, e
                            );
                        }
                    }
                }
            }
        }
    } else {
        let dm_pair = find_dm_either_channel(dm_channels, channel_id_u64).await;

        if let Some((bot_channel_id, user_channel_id)) = dm_pair {
            let pool = er!(
                get_pool_tokidm().await,
                "[ERROR] DM message_update user->bot get_pool_tokidm()"
            );
            let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

            if let Some(pos) = table_name.find('u') {
                if let Ok(bot_channel_id_u64) = table_name[1..pos].parse::<u64>() {
                    let mut qb: QueryBuilder<MySql> = QueryBuilder::new("SELECT bot FROM ");
                    qb.push(&table_name);
                    qb.push(" WHERE user = ");
                    qb.push_bind(event.id.get());

                    let result = qb.build_query_scalar::<u64>().fetch_one(pool).await;

                    if let Ok(bot_message_id) = result {
                        let message = build_edit_message(ctx, &event).await;

                        if let Err(e) = ChannelId::new(bot_channel_id_u64)
                            .edit_message(&ctx.http, MessageId::new(bot_message_id), message)
                            .await
                        {
                            eprintln!(
                                "[ERROR] Failed to send DM to user {}: {}",
                                bot_channel_id_u64, e
                            );
                        }
                    }
                }
            }
        }
    }
}
