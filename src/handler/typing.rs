use serenity::all::{ChannelId, TypingStartEvent};
use serenity::prelude::Context;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::constants::{BOT_USERID, GUILD_ID};
use crate::dm::{find_dm_bot_channel, find_dm_user_channel};

pub async fn handle_typing_start(
    ctx: &Context,
    event: TypingStartEvent,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
) {
    if event.user_id == BOT_USERID {
        return;
    }
    let channel_id_u64 = event.channel_id.get();

    if let Some(guild_id) = event.guild_id {
        if guild_id.get() != GUILD_ID.get() {
            return;
        }

        if let Some((_, user_channel_id)) = find_dm_bot_channel(dm_channels, channel_id_u64).await {
            if let Err(e) = ChannelId::new(user_channel_id)
                .broadcast_typing(&ctx.http)
                .await
            {
                eprintln!("[ERROR] send typing indicator: {:?}", e);
            }
        }
    } else if let Some((bot_channel_id, _)) =
        find_dm_user_channel(dm_channels, channel_id_u64).await
    {
        if let Err(e) = ChannelId::new(bot_channel_id)
            .broadcast_typing(&ctx.http)
            .await
        {
            eprintln!("[ERROR] send typing indicator: {:?}", e);
        }
    }
}
