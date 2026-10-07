use std::sync::Arc;

use coral_server::{
    mob::{Mob, MobRegistry, MobType},
    player::registry::PlayerRegistry,
};
use coral_types::MobSpawnBroadcast;
use tokio::sync::watch::Sender;

use crate::{Command, CommandResult, make_handler};

// FIXME: not implemented

pub fn command(
    player_registry: Arc<PlayerRegistry>,
    mob_registry: Arc<MobRegistry>,
    mob_spawn_tx: Arc<Sender<MobSpawnBroadcast>>,
    next_entity_id: i32,
) -> Command {
    Command {
        name: "summon",
        aliases: vec![],
        description: "Summon a mob",
        usage: "/summon <mob> [x, y, z]",
        handler: make_handler(move |ctx| {
            let player_registry = player_registry.clone();
            let mob_registry = mob_registry.clone();
            let mob_spawn_tx = mob_spawn_tx.clone();
            async move {
                if !ctx.is_op {
                    return CommandResult::Error("No permission!".to_string());
                }
                let Some(mob_name) = ctx.arg(1) else {
                    return CommandResult::Error("Usage: /summon <mob> [x, y, z]".to_string());
                };
                let Some(mob_type) = MobType::from_name(mob_name) else {
                    return CommandResult::Error(format!(
                        "Unknown mob: {}. Valid: {}",
                        mob_name,
                        MobType::ALL_NAMES
                    ));
                };

                let (x, y, z) = if ctx.args.len() >= 5 {
                    let parse = |s: &str| s.parse::<f64>().ok();
                    match (
                        parse(ctx.arg(2).unwrap_or("")),
                        parse(ctx.arg(3).unwrap_or("")),
                        parse(ctx.arg(4).unwrap_or("")),
                    ) {
                        (Some(x), Some(y), Some(z)) => (x, y, z),
                        _ => return CommandResult::Error("Invalid coordinates".to_string()),
                    }
                } else {
                    let players = player_registry.get_all().await;
                    let Some(sender) = players
                        .iter()
                        .find(|p| p.username.eq_ignore_ascii_case(&ctx.sender))
                    else {
                        return CommandResult::Error(
                            "Console must specify coodinates: /summon <mob> <x> <y> <z>"
                                .to_string(),
                        );
                    };
                    (sender.x, sender.y, sender.z)
                };

                let entity_id = next_entity_id;
                let mob = Mob::new(entity_id, mob_type.clone(), x, y, z);
                let health = mob.health;

                mob_registry.spawn(mob).await;

                mob_spawn_tx
                    .send((
                        entity_id,
                        mob_type.entity_id(),
                        x,
                        y,
                        z,
                        0.0,
                        0.0,
                        0.0,
                        health,
                    ))
                    .ok();

                CommandResult::Success(format!(
                    "Summoned {} at {:.1} {:.1} {:.1}",
                    mob_type.name(),
                    x,
                    y,
                    z
                ))
            }
        }),
    }
}
