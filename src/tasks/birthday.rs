use chrono::{Datelike, Duration, Local, Utc};
use serenity::all::{ChannelId, CreateMessage, Member, RoleId};
use serenity::http::Http;
use sqlx::{MySqlPool, Row};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::sleep;

use crate::constants::GUILD_ID;
use crate::db::{get_row_datetime, get_row_u64};

pub fn spawn_birthday_task(
    pool: MySqlPool,
    http: Arc<Http>,
    stop_flag: Arc<AtomicBool>,
    stop_notify: Arc<Notify>,
) {
    let birthday_id = RoleId::new(1299153941623341056);
    let birth_channel = ChannelId::new(1415398554700349592);

    tokio::spawn(async move {
        loop {
            let mut birth_users: Vec<Member> = Vec::new();
            let now_local = Local::now();
            let today = (now_local.month() as u16) * 100 + now_local.day() as u16;

            let birth_today_id_raw =
                sqlx::query("SELECT id, data, time, unused FROM birthday_user WHERE data = ?")
                    .bind(today)
                    .fetch_all(&pool)
                    .await
                    .unwrap_or_default();

            if !birth_today_id_raw.is_empty() {
                println!("[SYS] Birthday User(s) Found: {}", birth_today_id_raw.len());

                let mut user_ids_today: Vec<u64> = Vec::new();

                for row in birth_today_id_raw {
                    let row_id: u64 = get_row_u64(&row, "id").unwrap_or_default();
                    let time = get_row_datetime(&row, "time");
                    let unused_val: Option<i64> = row
                        .try_get::<bool, _>("unused")
                        .map(|b| if b { 1 } else { 0 })
                        .ok()
                        .or_else(|| row.try_get::<i64, _>("unused").ok())
                        .or_else(|| row.try_get::<i8, _>("unused").map(|v| v as i64).ok());

                    if let Some(t) = time {
                        if unused_val == Some(0) {
                            user_ids_today.push(row_id);
                        }

                        let now_utc = Utc::now();
                        if unused_val.is_some() && (now_utc - t).num_days() > 364 {
                            let _ = sqlx::query("DELETE FROM birthday_user WHERE id = ?")
                                .bind(row_id)
                                .execute(&pool)
                                .await;
                        }
                    }
                }

                match GUILD_ID.to_partial_guild(&http).await {
                    Ok(guild) => {
                        let mut falled_user_ids = Vec::new();
                        for user_id in user_ids_today {
                            match guild.member(&http, user_id).await {
                                Ok(member) => {
                                    if !member.roles.contains(&birthday_id) {
                                        birth_users.push(member);
                                    }
                                }
                                Err(e) => {
                                    let mut should_delete = true;

                                    if let serenity::all::Error::Http(
                                        serenity::all::HttpError::UnsuccessfulRequest(resp),
                                    ) = &e
                                    {
                                        let status = resp.status_code.as_u16();
                                        if status >= 500 || status == 429 {
                                            should_delete = false;
                                        }
                                    }

                                    if should_delete {
                                        let _ =
                                            sqlx::query("DELETE FROM birthday_user WHERE id = ?")
                                                .bind(user_id)
                                                .execute(&pool)
                                                .await;

                                        falled_user_ids.push(user_id);
                                    }
                                }
                            }
                        }

                        if !falled_user_ids.is_empty() {
                            println!(
                                "[SYS] Birthday User(s) Removed (Left the server): {}",
                                falled_user_ids.len()
                            );
                        }

                        if !birth_users.is_empty() {
                            let _ = birth_channel.broadcast_typing(&http).await;

                            let mut success_users_id = Vec::new();
                            for user in &birth_users {
                                match user.add_role(&http, birthday_id).await {
                                    Ok(_) => {
                                        success_users_id.push(user.user.id);
                                        println!(
                                            "[SYS] Birthday Role Added: {} ({})",
                                            user.user.id.get(),
                                            user.user.name
                                        );
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "[ERROR] Birthday add_role(): {:?} for {} ({})",
                                            e,
                                            user.user.name,
                                            user.user.id.get()
                                        );
                                    }
                                }
                            }

                            if !success_users_id.is_empty() {
                                let mut lines = String::new();

                                for user_id in &success_users_id {
                                    lines.push_str(&format!("- <@{}>\n", user_id));
                                }

                                let header = format!(
                                    "<@&1299153941623341056>\n🎉 ({})\n",
                                    success_users_id.len()
                                );

                                let full_content = format!("{}{}", header, lines);

                                let content = if full_content.len() > 2000 {
                                    format!("{}(Too many people's birthdays!)", header)
                                } else {
                                    full_content
                                };

                                let birth_message = CreateMessage::new().content(content);

                                if let Err(e) =
                                    birth_channel.send_message(&http, birth_message).await
                                {
                                    eprintln!("[ERROR] Birthday send_message(): {:?}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] Birthday GUILD_ID.to_partial_guild(): {:?}", e);
                    }
                }
            }

            let now = Local::now().naive_local();

            let mut target = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
            if now >= target {
                target += Duration::days(1);
            }
            let duration = target.signed_duration_since(now).to_std().unwrap();

            tokio::select! {
                _ = sleep(duration) => {}
                _ = stop_notify.notified() => {
                    if stop_flag.load(Ordering::Relaxed) {
                        println!("[SYS] Birth task Shutdown");
                        break;
                    }
                }
            }

            for user in &birth_users {
                let _ = user.remove_role(&http, birthday_id).await;
            }
            birth_users.clear();
        }
    });
}
