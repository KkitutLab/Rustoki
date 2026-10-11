use serenity::all::{ChannelId, CreateMessage, EditMessage, MessageId, Reaction};
use serenity::prelude::Context;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::constants::{BOT_USERID, CHANNELID_PIN, GUILD_ID};
use crate::db::{get_pool, get_pool_tokidm};
use crate::dm::{
    find_dm_bot_channel, find_dm_either_channel, find_dm_user_channel, find_message_pair,
};
use crate::macros::{er, sr};
use crate::upboard::{build_pin_embed, get_upboard_pin, update_upboard_count, update_upboard_pin};

pub async fn handle_reaction_add(
    ctx: &Context,
    ret: Reaction,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
) {
    let user_id = sr!(ret.user_id);

    if user_id == BOT_USERID {
        return;
    }

    if let Some(guild_id) = ret.guild_id {
        if guild_id.get() != GUILD_ID.get() {
            return;
        }

        let channel_id_u64 = ret.channel_id.get();

        let dm_pair = find_dm_bot_channel(dm_channels, channel_id_u64).await;

        if let Some((bot_channel_id, user_channel_id)) = dm_pair {
            let pool = er!(
                get_pool_tokidm().await,
                "[ERROR] DM reaction_add bot->user get_pool_tokidm()"
            );
            let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

            if let Ok(Some(row)) = find_message_pair(pool, &table_name, ret.message_id.get()).await
            {
                let target_message_id = row.target_for(ret.message_id.get());

                if let Err(e) = ChannelId::new(user_channel_id)
                    .create_reaction(&ctx.http, target_message_id, ret.emoji)
                    .await
                {
                    eprintln!(
                        "[ERROR] create_reaction in Channel {}: {:?}",
                        user_channel_id, e
                    );
                }
            }
            return;
        }

        let pool = er!(get_pool().await, "[ERROR] Upboard reaction_add get_pool()");

        match update_upboard_count(pool, ctx, &ret, true).await {
            Ok((msg_opt, count)) => {
                let lets_up_board = count >= 3;

                let message = match msg_opt {
                    Some(m) => m,
                    None => match ret.message(&ctx.http).await {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("[ERROR] get reaction original message(): {:?}", e);
                            return;
                        }
                    },
                };

                if lets_up_board {
                    match get_upboard_pin(pool, message.id.get()).await {
                        Ok(pin_id) => {
                            let embed = build_pin_embed(&message, count);

                            if pin_id == 0 {
                                let builder = CreateMessage::new().embed(embed);
                                if let Ok(pinmsg) =
                                    CHANNELID_PIN.send_message(&ctx.http, builder).await
                                {
                                    let _ = update_upboard_pin(
                                        pool,
                                        ret.message_id.get(),
                                        pinmsg.id.get(),
                                    )
                                    .await;
                                }
                            } else {
                                let builder = EditMessage::new().embed(embed);
                                let _ = CHANNELID_PIN
                                    .edit_message(&ctx.http, MessageId::new(pin_id), builder)
                                    .await;
                            }
                        }
                        Err(e) => {
                            eprintln!("[ERROR] get_upboard_pin(): {:?}", e);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[ERROR] update_upboard_count: {:?}", e);
            }
        }
    } else {
        let channel_id_u64 = ret.channel_id.get();

        let dm_pair = find_dm_user_channel(dm_channels, channel_id_u64).await;

        if let Some((bot_channel_id, user_channel_id)) = dm_pair {
            let pool = er!(
                get_pool_tokidm().await,
                "[ERROR] DM reaction_add user->bot get_pool_tokidm()"
            );
            let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

            if let Ok(Some(row)) = find_message_pair(pool, &table_name, ret.message_id.get()).await
            {
                let target_message_id = row.target_for(ret.message_id.get());

                if let Err(e) = ChannelId::new(bot_channel_id)
                    .create_reaction(&ctx.http, target_message_id, ret.emoji)
                    .await
                {
                    eprintln!(
                        "[ERROR] create_reaction in Channel {}: {:?}",
                        bot_channel_id, e
                    );
                }
            }
        }
    }
}

pub async fn handle_reaction_remove(
    ctx: &Context,
    ret: Reaction,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
) {
    let user_id = sr!(ret.user_id);

    if user_id == BOT_USERID {
        return;
    }

    let channel_id_u64 = ret.channel_id.get();

    let dm_pair = find_dm_either_channel(dm_channels, channel_id_u64).await;

    if let Some((bot_channel_id, user_channel_id)) = dm_pair {
        let pool = er!(
            get_pool_tokidm().await,
            "[ERROR] DM reaction_remove get_pool_tokidm()"
        );
        let table_name = format!("c{}u{}", bot_channel_id, user_channel_id);

        if let Ok(Some(row)) = find_message_pair(pool, &table_name, ret.message_id.get()).await {
            let target_message_id = row.target_for(ret.message_id.get());

            let target_channel_id = if channel_id_u64 == bot_channel_id {
                user_channel_id
            } else {
                bot_channel_id
            };

            if let Err(e) = ChannelId::new(target_channel_id)
                .delete_reaction(
                    &ctx.http,
                    MessageId::new(target_message_id),
                    Some(BOT_USERID),
                    ret.emoji,
                )
                .await
            {
                eprintln!(
                    "[ERROR] remove reaction in Channel {}: {:?}",
                    target_channel_id, e
                );
            }
        }

        return;
    }

    if let Some(guild_id) = ret.guild_id {
        if guild_id.get() != GUILD_ID.get() {
            return;
        }

        let pool = er!(
            get_pool().await,
            "[ERROR] Upboard reaction_remove get_pool()"
        );

        match update_upboard_count(pool, ctx, &ret, false).await {
            Ok((_, count)) => {
                let lets_delete_board = count < 3;

                match get_upboard_pin(pool, ret.message_id.get()).await {
                    Ok(pin_id) => {
                        if pin_id != 0 {
                            if lets_delete_board {
                                if let Err(e) = CHANNELID_PIN
                                    .delete_message(&ctx.http, MessageId::new(pin_id))
                                    .await
                                {
                                    eprintln!("[ERROR] Failed to delete pin message: {}", e);
                                } else {
                                    let _ = update_upboard_pin(pool, ret.message_id.get(), 0).await;
                                }
                            } else {
                                match ret.message(&ctx.http).await {
                                    Ok(original_message) => {
                                        let embed = build_pin_embed(&original_message, count);
                                        let builder = EditMessage::new().embed(embed);
                                        let _ = CHANNELID_PIN
                                            .edit_message(
                                                &ctx.http,
                                                MessageId::new(pin_id),
                                                builder,
                                            )
                                            .await;
                                    }
                                    Err(e) => {
                                        eprintln!("[ERROR] get pin original message(): {:?}", e);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] get_upboard_pin(): {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("[ERROR] update_upboard_count: {:?}", e);
            }
        }
    }
}
