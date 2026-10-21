use serenity::all::Interaction;
use serenity::prelude::Context;
use sqlx::MySqlPool;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::commands::slash::delete::DeleteMessagesStore;
use crate::commands::slash::trap::TrapIdsStore;
use crate::commands::slash::xfix::XfixStore;
use crate::commands::slash::{dispatch_component, dispatch_slash_command};
use crate::response::Resp;

#[allow(clippy::too_many_arguments)]
pub async fn handle_interaction(
    ctx: &Context,
    interaction: Interaction,
    pool: &MySqlPool,
    resp: &Arc<Mutex<Vec<Resp>>>,
    blacklist_guilds: &Arc<Mutex<Vec<u64>>>,
    delete_messages: &DeleteMessagesStore,
    dm_channels: &Arc<Mutex<Vec<(u64, u64)>>>,
    trap_ids: &TrapIdsStore,
    xfix: &XfixStore,
) {
    match interaction {
        Interaction::Command(command) => {
            dispatch_slash_command(
                ctx,
                &command,
                pool,
                resp,
                blacklist_guilds,
                delete_messages,
                dm_channels,
                trap_ids,
                xfix,
            )
            .await;
        }
        Interaction::Component(component) => {
            dispatch_component(ctx, &component, pool).await;
        }
        _ => {}
    }
}
