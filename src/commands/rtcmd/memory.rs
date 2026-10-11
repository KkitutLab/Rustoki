use serenity::all::Message;
use serenity::prelude::Context;

use crate::utils::system::get_vmrss_mb;

pub async fn handle_memory(ctx: &Context, msg: &Message) {
    let vmrss = get_vmrss_mb().await;
    let resp = match vmrss {
        Ok(mb) => format!("{:.2} MB", mb),
        Err(e) => format!("Failed to get VmRSS: {}", e),
    };
    if let Err(e) = msg.channel_id.say(&ctx.http, resp).await {
        eprintln!("[ERROR] <! say(): {:?}", e);
    }
}
