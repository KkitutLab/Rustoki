use serenity::all::{Color, EditRole, GuildMemberUpdateEvent, Member, Permissions, Role, RoleId};
use serenity::prelude::Context;
use std::collections::HashMap;

use crate::utils::image::{average_center_color, fetch_image_from_url};

async fn remove_color_roles(
    ctx: &Context,
    member: &Member,
    roles_map: &HashMap<RoleId, Role>,
) -> Result<(), ()> {
    for role_id in &member.roles {
        if let Some(role) = roles_map.get(role_id) {
            if role.name.starts_with(':') {
                if let Err(e) = member.remove_role(&ctx.http, *role_id).await {
                    eprintln!("[ERROR] remove_role: {:?}", e);
                    return Err(());
                }
            }
        }
    }
    Ok(())
}

pub async fn handle_guild_member_update(
    ctx: &Context,
    old: Option<Member>,
    event: GuildMemberUpdateEvent,
) {
    let target_role = RoleId::new(1366815072982401064);
    let mut change_auto_color = false;

    match &old {
        None => {
            if event.roles.contains(&target_role) {
                change_auto_color = true;
            }
        }
        Some(old_member) => {
            let had_role = old_member.roles.contains(&target_role);
            let has_role = event.roles.contains(&target_role);

            match (had_role, has_role) {
                (false, true) => {
                    change_auto_color = true;
                }
                (true, true) => {
                    if old_member.user.avatar != event.user.avatar {
                        change_auto_color = true;
                    }
                }
                (true, false) => match event.guild_id.roles(&ctx.http).await {
                    Ok(roles_map) => {
                        let member = match event.guild_id.member(&ctx.http, event.user.id).await {
                            Ok(m) => m,
                            Err(e) => {
                                eprintln!("[ERROR] fetch member: {:?}", e);
                                return;
                            }
                        };
                        let _ = remove_color_roles(ctx, &member, &roles_map).await;
                    }
                    Err(e) => {
                        eprintln!("[ERROR] fetch roles: {:?}", e);
                        return;
                    }
                },
                (false, false) => {
                    return;
                }
            }
        }
    }

    if change_auto_color {
        let avatar_url = event
            .user
            .avatar_url()
            .unwrap_or_else(|| event.user.default_avatar_url());

        match fetch_image_from_url(&avatar_url).await {
            Ok(img) => {
                let color = average_center_color(&img);
                let hex_color = format!("{:02x}{:02x}{:02x}", color.0[0], color.0[1], color.0[2]);

                match event.guild_id.roles(&ctx.http).await {
                    Ok(roles_map) => {
                        let existing_role = roles_map.values().find(|r| {
                            let name = r.name.to_lowercase();
                            name.len() >= 6 && name[name.len() - 6..] == hex_color
                        });

                        let member = match event.guild_id.member(&ctx.http, event.user.id).await {
                            Ok(m) => m,
                            Err(e) => {
                                eprintln!("[ERROR] event.guild_id.member: {:?}", e);
                                return;
                            }
                        };

                        if existing_role
                            .as_ref()
                            .map(|r| member.roles.contains(&r.id))
                            .unwrap_or(false)
                        {
                            return;
                        }

                        if remove_color_roles(ctx, &member, &roles_map).await.is_err() {
                            return;
                        }

                        let new_role = if let Some(role) = existing_role {
                            role.clone()
                        } else {
                            let roles_map = match event.guild_id.roles(&ctx.http).await {
                                Ok(map) => map,
                                Err(e) => {
                                    eprintln!("[ERROR] roles after creation: {:?}", e);
                                    return;
                                }
                            };

                            let autoindex_role_id = RoleId::new(1366828051857543228);
                            let mut sorted_roles: Vec<_> = roles_map.iter().collect();
                            sorted_roles.sort_by_key(|(_, r)| r.position);

                            let mut target_position = 0;
                            if let Some((idx, _)) = sorted_roles
                                .iter()
                                .enumerate()
                                .find(|(_, (&id, _))| id == autoindex_role_id)
                            {
                                target_position = sorted_roles[idx].1.position + 1;
                            }

                            let [r, g, b] = color.0;
                            let rolebuilder = EditRole::new()
                                .colour(Color::from_rgb(r, g, b))
                                .name(format!(":{}", hex_color))
                                .position(target_position)
                                .permissions(Permissions::empty());

                            match event.guild_id.create_role(&ctx.http, rolebuilder).await {
                                Ok(role) => role,
                                Err(e) => {
                                    eprintln!("[ERROR] create_role: {:?}", e);
                                    return;
                                }
                            }
                        };

                        if let Err(e) = member.add_role(&ctx.http, new_role.id).await {
                            eprintln!("[ERROR] add_role: {:?}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("[ERROR] event.guild_id.roles: {:?}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("[ERROR] fetch_image_from_url: {:?}", e);
            }
        }
    }
}
