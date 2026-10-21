pub mod interaction;
pub mod member;
pub mod message;
pub mod message_events;
pub mod reaction;
pub mod ready;
pub mod typing;

use serenity::all::{
    ChannelId, GuildId, GuildMemberUpdateEvent, Interaction, Member, Message, MessageId,
    MessageUpdateEvent, Reaction, Ready, TypingStartEvent,
};
use serenity::async_trait;
use serenity::prelude::{Context, EventHandler};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

use crate::commands::slash::delete::DeleteMessagesStore;
use crate::commands::slash::trap::TrapIdsStore;
use crate::commands::slash::xfix::XfixStore;
use crate::db::get_pool;
use crate::macros::er;
use crate::response::{Resp, RespStore};
use interaction::handle_interaction;
use member::handle_guild_member_update;
use message::handle_message;
use message_events::{handle_message_delete, handle_message_update};
use reaction::{handle_reaction_add, handle_reaction_remove};
use ready::handle_ready;
use typing::handle_typing_start;

pub struct Handler {
    pub is_ready: Arc<AtomicBool>,
    pub delete_messages: DeleteMessagesStore, //user, first, end, time. tuple(id, guild, channel)
    pub stop_flag: Arc<AtomicBool>,
    pub stop_notify: Arc<Notify>,
    pub resp: Arc<Mutex<Vec<Resp>>>,
    pub resp_store: Arc<RespStore>,
    pub blacklist_guilds: Arc<Mutex<Vec<u64>>>,
    pub dm_channels: Arc<Mutex<Vec<(u64, u64)>>>,
    pub trap_ids: TrapIdsStore,
    pub xfix_channels: XfixStore,
}

#[async_trait]
impl EventHandler for Handler {
    async fn guild_member_update(
        &self,
        ctx: Context,
        old: Option<Member>,
        _: Option<Member>,
        event: GuildMemberUpdateEvent,
    ) {
        handle_guild_member_update(&ctx, old, event).await;
    }

    async fn typing_start(&self, ctx: Context, event: TypingStartEvent) {
        handle_typing_start(&ctx, event, &self.dm_channels).await;
    }

    async fn reaction_add(&self, ctx: Context, ret: Reaction) {
        handle_reaction_add(&ctx, ret, &self.dm_channels).await;
    }

    async fn reaction_remove(&self, ctx: Context, ret: Reaction) {
        handle_reaction_remove(&ctx, ret, &self.dm_channels).await;
    }

    async fn message_delete(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        message_id: MessageId,
        guild_id: Option<GuildId>,
    ) {
        handle_message_delete(&ctx, channel_id, message_id, guild_id, &self.dm_channels).await;
    }

    async fn message_update(
        &self,
        ctx: Context,
        _old: Option<Message>,
        _new: Option<Message>,
        event: MessageUpdateEvent,
    ) {
        handle_message_update(&ctx, event, &self.dm_channels).await;
    }

    async fn message(&self, ctx: Context, msg: Message) {
        handle_message(
            &ctx,
            msg,
            &self.resp,
            &self.resp_store,
            &self.blacklist_guilds,
            &self.dm_channels,
            &self.trap_ids,
            &self.xfix_channels,
            &self.stop_flag,
            &self.stop_notify,
        )
        .await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let pool = er!(get_pool().await, "[ERROR] interaction get_pool()");
        handle_interaction(
            &ctx,
            interaction,
            pool,
            &self.resp,
            &self.blacklist_guilds,
            &self.delete_messages,
            &self.dm_channels,
            &self.trap_ids,
            &self.xfix_channels,
        )
        .await;
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let pool = er!(get_pool().await, "[ERROR] ready get_pool()");
        handle_ready(
            &ctx,
            ready,
            &self.is_ready,
            &self.stop_flag,
            &self.stop_notify,
            pool,
        )
        .await;
    }
}
