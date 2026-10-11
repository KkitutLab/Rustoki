use serenity::all::{
    CommandInteraction, CreateEmbed, CreateInteractionResponseFollowup, Timestamp,
};
use serenity::prelude::Context;
use sha2::Digest;

use crate::utils::lng_trs;

pub async fn handle_match(ctx: &Context, command: &CommandInteraction, lng: bool) {
    if let Err(e) = command.defer(&ctx.http).await {
        eprintln!("[ERROR] Command defer(): {:?}", e);
        return;
    }

    let user1 = command
        .data
        .options
        .first()
        .and_then(|o| o.value.as_user_id());
    let user2 = command
        .data
        .options
        .get(1)
        .and_then(|o| o.value.as_user_id());

    match (user1, user2) {
        (Some(u1_id), Some(u2_id)) => {
            let id1 = u1_id.get();
            let id2 = u2_id.get();
            let score = calculate_match_score(id1, id2);

            let embed = CreateEmbed::new()
                .title("Match Result")
                .description(format!(
                    "### **<@{}>** <-> **<@{}>** : **{}%**",
                    id1, id2, score
                ))
                .color(0xFFAAAA)
                .timestamp(Timestamp::now());

            if let Err(e) = command
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new().add_embed(embed),
                )
                .await
            {
                eprintln!("[ERROR] match command response: {:?}", e);
            }
        }
        _ => {
            if let Err(e) = command
                .create_followup(
                    &ctx.http,
                    CreateInteractionResponseFollowup::new()
                        .content(lng_trs(
                            lng,
                            "Please select both users.",
                            "두 유저를 모두 선택해주세요.",
                        ))
                        .ephemeral(true),
                )
                .await
            {
                eprintln!("[ERROR] Command create_followup(): {:?}", e);
            }
        }
    }
}

pub fn calculate_match_score(id1: u64, id2: u64) -> u32 {
    let (low_id, high_id) = if id1 < id2 { (id1, id2) } else { (id2, id1) };

    let combined_id_str = format!("{}{}", low_id, high_id);
    let hash_str = hex::encode(sha2::Sha256::digest(combined_id_str.as_bytes()));

    let char1 = hash_str.chars().nth(12).unwrap();
    let char2 = hash_str.chars().nth(13).unwrap();
    let combined_hex = format!("{}{}", char1, char2);

    let combined_val = u16::from_str_radix(&combined_hex, 16).unwrap_or(0);
    ((combined_val as f32 / 255.0) * 100.0).round() as u32
}
