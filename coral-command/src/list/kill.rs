use std::sync::Arc;

use coral_server::player::registry::PlayerRegistry;
use coral_types::DamageEvent;
use tokio::sync::broadcast::Sender;

use crate::{Command, CommandResult, make_handler};

pub fn command(player_registry: Arc<PlayerRegistry>, dmg_tx: Arc<Sender<DamageEvent>>) -> Command {
    Command {
        name: "kill",
        aliases: vec![],
        description: "Kill yourself or another player",
        usage: "/kill [player]",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let tx = dmg_tx.clone();
            async move {
                if !ctx.is_op {
                    return CommandResult::Error("No permission.".to_string());
                }
                let target_name = ctx.arg(1).unwrap_or(&ctx.sender);
                let players = registry.get_all().await;
                let Some(target) = players
                    .iter()
                    .find(|p| p.username.to_lowercase() == target_name.to_lowercase())
                else {
                    return CommandResult::Error(format!("Player not found: {}", target_name));
                };

                registry
                    .update_health(&target.uuid, 0.0, target.food, target.food_saturation)
                    .await;
                tx.send((
                    target.uuid,
                    0.0,
                    target.food,
                    target.food_saturation,
                    target.entity_id,
                ))
                .ok();

                CommandResult::Broadcast(format!("{} was killed", target.username))
            }
        }),
    }
}
