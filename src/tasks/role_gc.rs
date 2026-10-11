use chrono::{Duration, Local};
use serenity::all::{CreateEmbed, CreateMessage};
use serenity::http::Http;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::sleep;

use crate::constants::{CHANNELID_LOG_SECRET, GUILD_ID};

async fn send_rolegc_fail<E: std::fmt::Debug>(http: &Arc<Http>, target: &str, err: &E) {
    let builder = CreateMessage::new().embed(
        CreateEmbed::new()
            .title("RoleGC FAIL")
            .description(format!("Failed to fetch {}\n```{:?}```", target, err))
            .color(0xFF4444),
    );
    if let Err(e) = CHANNELID_LOG_SECRET.send_message(http, builder).await {
        eprintln!("[ERROR] Failed to send RoleGC FAIL message: {:?}", e);
    }
}

pub fn spawn_role_gc_task(http: Arc<Http>, stop_flag: Arc<AtomicBool>, stop_notify: Arc<Notify>) {
    tokio::spawn(async move {
        loop {
            let today_naive = Local::now().date_naive();
            let next_midnight = (today_naive + Duration::days(7))
                .and_hms_opt(0, 0, 0)
                .unwrap();

            let remaining_time = next_midnight.signed_duration_since(Local::now().naive_local());
            let duration = remaining_time
                .to_std()
                .unwrap_or_else(|_| std::time::Duration::from_secs(7 * 24 * 3600));

            tokio::select! {
                _ = sleep(duration) => {}
                _ = stop_notify.notified() => {
                    if stop_flag.load(Ordering::Relaxed) {
                        println!("[SYS] RoleGC task Shutdown");
                        break;
                    }
                }
            }

            println!("[SYS] RoleGC RUN");

            let builder =
                CreateMessage::new().embed(CreateEmbed::new().title("RoleGC RUN").color(0xFFAAAA));
            if let Err(e) = CHANNELID_LOG_SECRET.send_message(&http, builder).await {
                eprintln!("[ERROR] Failed to send RoleGC RUN message: {:?}", e);
            }

            let mut manage_roles = Vec::new();
            match GUILD_ID.roles(&http).await {
                Ok(roles) => {
                    for (role_id, role) in roles {
                        if role.name.starts_with(':') {
                            manage_roles.push((role_id, role.name));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[ERROR] Failed to fetch roles: {:?}", e);
                    send_rolegc_fail(&http, "roles", &e).await;
                    return;
                }
            }

            match GUILD_ID.members(&http, None, None).await {
                Ok(members) => {
                    manage_roles.retain(|(role_id, _name)| {
                        !members.iter().any(|member| member.roles.contains(role_id))
                    });
                }
                Err(e) => {
                    eprintln!("[ERROR] Failed to fetch members: {:?}", e);
                    send_rolegc_fail(&http, "members", &e).await;
                    return;
                }
            }

            let mut deleted_role_names = Vec::new();
            let mut failed_role_names = Vec::new();
            for role_id in manage_roles {
                if let Err(e) = GUILD_ID.delete_role(&http, role_id.0).await {
                    eprintln!("[ERROR] Failed to delete role {}: {:?}", role_id.0, e);
                    failed_role_names.push(role_id.1);
                } else {
                    deleted_role_names.push(role_id.1);
                }
            }

            let end_message = match (deleted_role_names.is_empty(), failed_role_names.is_empty()) {
                (true, true) => {
                    println!("[SYS] No roles processed");
                    String::from("```No roles deleted or failed```")
                }
                (false, true) => {
                    let roles_str = deleted_role_names
                        .iter()
                        .map(|r| format!("  {}", r))
                        .collect::<Vec<_>>()
                        .join("\n");
                    println!("[SYS] [Deleted roles]\n{}", roles_str);
                    format!("```[Deleted roles]\n{}\n```", roles_str)
                }
                (true, false) => {
                    let roles_str = failed_role_names
                        .iter()
                        .map(|r| format!("  {}", r))
                        .collect::<Vec<_>>()
                        .join("\n");
                    println!("[SYS] [Failed roles]\n{}", roles_str);
                    format!("```[Failed roles]\n{}\n```", roles_str)
                }
                (false, false) => {
                    let ok_str = deleted_role_names
                        .iter()
                        .map(|r| format!("  {}", r))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let fail_str = failed_role_names
                        .iter()
                        .map(|r| format!("  {}", r))
                        .collect::<Vec<_>>()
                        .join("\n");
                    println!(
                        "[SYS] [Deleted roles]\n{}\n[Failed roles]\n{}",
                        ok_str, fail_str
                    );
                    format!(
                        "```[Deleted roles]\n{}\n\n[Failed roles]\n{}\n```",
                        ok_str, fail_str
                    )
                }
            };

            let builder = CreateMessage::new().embed(
                CreateEmbed::new()
                    .title("RoleGC RESULT")
                    .description(end_message)
                    .color(0xFFAAAA),
            );

            if let Err(e) = CHANNELID_LOG_SECRET.send_message(&http, builder).await {
                eprintln!("[ERROR] Failed to send RoleGC END message: {:?}", e);
            }
        }
    });
}
