use serenity::all::{
    CommandInteraction, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use serenity::prelude::Context;

use crate::utils::lng_trs;

pub async fn handle_inquiry(ctx: &Context, command: &CommandInteraction, lng: bool) {
    if let Err(e) = command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(
                        CreateEmbed::new()
                            .title(lng_trs(lng, "please DM!", "DM ㄱㄱ!"))
                            .description(lng_trs(
                                lng,
                                "Simply send a DM to the bot to use it!",
                                "단순히 봇에게 DM을 보내서 이용해줘!",
                            ))
                            .color(0xFFAAAA),
                    )
                    .ephemeral(true),
            ),
        )
        .await
    {
        eprintln!("[ERROR] Command create_response(): {:?}", e);
    }
}
