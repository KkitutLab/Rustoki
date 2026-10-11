use chrono::{Duration, Local, Timelike, Utc};
use serenity::all::{
    ButtonStyle, CommandInteraction, ComponentInteraction, CreateButton, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
};
use serenity::prelude::Context;
use sqlx::MySqlPool;

use crate::db::get_row_datetime;
use crate::utils::lng_trs;

pub async fn handle_birthday(
    ctx: &Context,
    command: &CommandInteraction,
    pool: &MySqlPool,
    lng: bool,
) {
    let now = Local::now().time();
    if (now.hour() == 23 && now.minute() >= 59) || (now.hour() == 0 && now.minute() == 0) {
        if let Err(e) = command
            .create_response(
                &ctx.http,
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(lng_trs(lng, "It is maintenance time", "점검 시간이야"))
                        .ephemeral(true),
                ),
            )
            .await
        {
            eprintln!("[ERROR] Command create_response(): {:?}", e);
        }
    }
    if let Err(e) = command.defer_ephemeral(&ctx.http).await {
        eprintln!("[ERROR] Command defer(): {:?}", e);
        return;
    }

    let respstr;
    let user_id = command.user.id.get();

    if let Some(opt) = command.data.options.first() {
        if let Some(mmdd) = opt.value.as_i64() {
            let row_opt = sqlx::query("SELECT data, time FROM birthday_user WHERE id = ?")
                .bind(user_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

            let is_okuser = if let Some(record) = row_opt {
                let time_opt = get_row_datetime(&record, "time");
                if let Some(t) = time_opt {
                    if t < (Utc::now() - Duration::days(364)) {
                        1
                    } else {
                        2
                    }
                } else {
                    1
                }
            } else {
                0
            };

            if is_okuser > 1 {
                respstr = lng_trs(
                    lng,
                    "You can only set your birthday once a year. If a reset is required, please contact the administrator",
                    "1년에 한번만 생일을 설정할 수 있어. 재설정이 필요하다면 담당자에게 문의해",
                );
            } else {
                let month = (mmdd / 100) as u32;
                let day = (mmdd % 100) as u32;

                let is_wrong_month = !(1..=12).contains(&month);
                let is_wrong_day = !(1..=31).contains(&day);

                if is_wrong_month && is_wrong_day {
                    respstr = lng_trs(lng, "Invalid date", "잘못된 날짜야");
                } else if is_wrong_day {
                    respstr = lng_trs(lng, "Invalid day", "잘못된 일자야");
                } else if is_wrong_month {
                    respstr = lng_trs(lng, "Invalid month", "잘못된 월이야");
                } else if month == 2 && day > 29 {
                    respstr = lng_trs(
                        lng,
                        "February cannot exceed 29 days",
                        "2월은 29일을 초과할 수 없어",
                    );
                } else if month == 2 && day == 29 {
                    let suffix = if is_okuser == 1 { "_update" } else { "" };
                    let bt1 = CreateButton::new(format!(
                        "birthday_leap_forward{}:{}",
                        suffix,
                        command.user.id.get()
                    ))
                    .label("2/28")
                    .style(ButtonStyle::Primary);
                    let bt2 = CreateButton::new(format!(
                        "birthday_leap_keep{}:{}",
                        suffix,
                        command.user.id.get()
                    ))
                    .label(lng_trs(lng, "KEEP", "유지"))
                    .style(ButtonStyle::Secondary);
                    let bt3 = CreateButton::new(format!(
                        "birthday_leap_backward{}:{}",
                        suffix,
                        command.user.id.get()
                    ))
                    .label("3/1")
                    .style(ButtonStyle::Primary);
                    let embed = CreateEmbed::new()
                        .title(lng_trs(lng, "February 29th", "2월 29일"))
                        .color(0xFFAAAA)
                        .description(lng_trs(
                            lng,
                            "February 29th is a leap year.\nYou can push the date forward or backward or **keep it as is**",
                            "2월 29일은 윤년이야.\n날짜를 당기거나 밀거나 **그대로 진행**할 수 있어",
                        ));
                    if let Err(e) = command
                        .create_followup(
                            &ctx.http,
                            CreateInteractionResponseFollowup::new()
                                .add_embed(embed)
                                .button(bt1)
                                .button(bt2)
                                .button(bt3),
                        )
                        .await
                    {
                        eprintln!("[ERROR] birthday_set create_followup(): {:?}", e);
                    }
                    return;
                } else {
                    let mmdd_num = format!("{:02}{:02}", month, day).parse::<u16>().unwrap();

                    if is_okuser == 1 {
                        sqlx::query("UPDATE birthday_user SET data = ? WHERE id = ?")
                            .bind(mmdd_num)
                            .bind(user_id)
                            .execute(pool)
                            .await
                            .unwrap();

                        respstr = lng_trs(lng, "Birthday is updated", "생일이 업데이트되었어");
                    } else {
                        sqlx::query("INSERT INTO birthday_user (id, data) VALUES (?, ?)")
                            .bind(user_id)
                            .bind(mmdd_num)
                            .execute(pool)
                            .await
                            .unwrap();

                        respstr = lng_trs(lng, "Birthday is set", "생일이 설정되었어");
                    }
                }
            }
        } else {
            respstr = lng_trs(lng, "Invalid value", "올바르지 않은 값이야");
        }
    } else {
        let record_opt = sqlx::query("SELECT time, unused FROM birthday_user WHERE id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

        if let Some(record) = record_opt {
            let time_opt = get_row_datetime(&record, "time");
            let is_old = time_opt
                .map(|t| t < (Utc::now() - Duration::days(364)))
                .unwrap_or(true);

            if is_old {
                sqlx::query("DELETE FROM birthday_user WHERE id = ?")
                    .bind(user_id)
                    .execute(pool)
                    .await
                    .unwrap();
                respstr = lng_trs(lng, "Removed successfully", "성공적으로 제거되었어");
            } else {
                sqlx::query("UPDATE birthday_user SET unused = true WHERE id = ?")
                    .bind(user_id)
                    .execute(pool)
                    .await
                    .unwrap();
                respstr = lng_trs(lng, "Removed successfully", "성공적으로 제거되었어");
            }
        } else {
            respstr = lng_trs(lng, "I couldn't find you", "너를 찾을 수 없었어");
        }
    }

    if let Err(e) = command
        .create_followup(
            &ctx.http,
            CreateInteractionResponseFollowup::new().content(respstr),
        )
        .await
    {
        eprintln!(
            "[ERROR] Command create_followup() at line {}: {:?}",
            line!(),
            e
        );
    }
}

pub async fn handle_birthday_button(
    ctx: &Context,
    component: &ComponentInteraction,
    pool: &MySqlPool,
    custom_id_prefix: &str,
    lng: bool,
) {
    let user_id = component.user.id.get();
    let mmdd: u16 = if custom_id_prefix == "birthday_leap_forward"
        || custom_id_prefix == "birthday_leap_forward_update"
    {
        228
    } else if custom_id_prefix == "birthday_leap_keep"
        || custom_id_prefix == "birthday_leap_keep_update"
    {
        229
    } else {
        301
    };

    let message = if custom_id_prefix.ends_with("_update") {
        lng_trs(lng, "Birthday is updated", "생일이 업데이트되었어")
    } else {
        lng_trs(lng, "Birthday is set", "생일이 설정되었어")
    };

    if custom_id_prefix.ends_with("_update") {
        sqlx::query("UPDATE birthday_user SET data = ? WHERE id = ?")
            .bind(mmdd)
            .bind(user_id)
            .execute(pool)
            .await
            .unwrap();
    } else {
        sqlx::query(
            "INSERT INTO birthday_user (id, data) VALUES (?, ?) ON DUPLICATE KEY UPDATE data = ?",
        )
        .bind(user_id)
        .bind(mmdd)
        .bind(mmdd)
        .execute(pool)
        .await
        .unwrap();
    }

    let embed = CreateEmbed::new().title(message).color(0x00FF00);

    if let Err(e) = component
        .create_response(
            &ctx.http,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .embed(embed)
                    .components(vec![]),
            ),
        )
        .await
    {
        eprintln!("[ERROR] Button birthday_leap_: {:?}", e);
    }
}
