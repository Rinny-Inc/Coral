use std::{
    path::{Component, Path, PathBuf},
    sync::{Arc, atomic::AtomicI64},
};

use coral_command::{
    CommandDispatcher,
    list::{self, usage::ResourceMonitor},
};
use coral_protocol::packets::play::{
    entity::{EntityAnimationType, EntityMetadata},
    game::EntityStatusType,
    movement::MovementBroadcast,
};
use coral_server::{
    ops::OpsFile,
    player::{Player, registry::PlayerRegistry},
    projectile::ProjectileKind,
    whitelist::WhitelistFile,
};
use coral_types::{
    BedUpdate, BlockUpdate, BreakAnimation, ChestAnimation, DamageEvent, DespawnEntity,
    EntityVelocityUpdate, EquipmentUpdate, GamemodeUpdate, ItemDrop, ItemPickup, KickRequest,
    ParticleEffect, PingUpdate, PrivateMessage, ProjectileMove, SignUpdate, SoundEffect,
    SplashEffect, TeleportRequest, TimeUpdate, XpOrbMove, XpOrbSpawn, XpPickup,
};
use coral_world::weather::WeatherState;
use tokio::sync::{
    RwLock,
    broadcast::{Sender, channel},
};

pub type JoinLeave = (Player, bool);
type AnimationUpdate = (i32, EntityAnimationType);
type EntityStatusUpdate = (i32, EntityStatusType);
type ProjectileSpawn = (i32, i32, ProjectileKind, f64, f64, f64, f64, f64, f64);

#[derive(Clone)]
pub struct Channels {
    pub chat_tx: Arc<Sender<String>>,
    pub join_tx: Arc<Sender<JoinLeave>>,
    pub pos_tx: Arc<Sender<MovementBroadcast>>,
    pub gm_tx: Arc<Sender<GamemodeUpdate>>,
    pub ping_tx: Arc<Sender<PingUpdate>>,
    pub block_tx: Arc<Sender<BlockUpdate>>,
    pub break_tx: Arc<Sender<BreakAnimation>>,
    pub anim_tx: Arc<Sender<AnimationUpdate>>,
    pub meta_tx: Arc<Sender<EntityMetadata>>,
    pub dmg_tx: Arc<Sender<DamageEvent>>,
    pub item_tx: Arc<Sender<ItemDrop>>,
    pub despawn_tx: Arc<Sender<DespawnEntity>>,
    pub pickup_tx: Arc<Sender<ItemPickup>>,
    pub time_tx: Arc<Sender<TimeUpdate>>,
    pub weather_tx: Arc<Sender<WeatherState>>,
    pub tick_tx: Arc<Sender<()>>,
    pub status_tx: Arc<Sender<EntityStatusUpdate>>,
    pub equip_tx: Arc<Sender<EquipmentUpdate>>,
    pub sound_tx: Arc<Sender<SoundEffect>>,
    pub shutdown_tx: Arc<Sender<()>>,
    pub particle_tx: Arc<Sender<ParticleEffect>>,
    pub projectile_spawn_tx: Arc<Sender<ProjectileSpawn>>,
    pub projectile_move_tx: Arc<Sender<ProjectileMove>>,
    pub splash_effect_tx: Arc<Sender<SplashEffect>>,
    pub xp_orb_spawn_tx: Arc<Sender<XpOrbSpawn>>,
    pub xp_orb_move_tx: Arc<Sender<XpOrbMove>>,
    pub xp_pickup_tx: Arc<Sender<XpPickup>>,
    pub bed_tx: Arc<Sender<BedUpdate>>,
    pub wake_tx: Arc<Sender<()>>,
    pub private_msg_tx: Arc<Sender<PrivateMessage>>,
    pub teleport_rq_tx: Arc<Sender<TeleportRequest>>,
    pub kick_rq_tx: Arc<Sender<KickRequest>>,
    pub sign_update_tx: Arc<Sender<SignUpdate>>,
    pub velocity_broadcast_tx: Arc<Sender<EntityVelocityUpdate>>,
    pub chest_anim_tx: Arc<Sender<ChestAnimation>>,
    pub furnace_update_tx: Arc<Sender<(i32, i32, i32)>>,
    pub difficulty_tx: Arc<Sender<u8>>,
    // TODO: mob_spawn_tx
}
impl Channels {
    pub fn new() -> Self {
        Self {
            chat_tx: Arc::new(channel::<String>(50).0),
            join_tx: Arc::new(channel::<JoinLeave>(16).0),
            pos_tx: Arc::new(channel::<MovementBroadcast>(100).0),
            gm_tx: Arc::new(channel::<GamemodeUpdate>(16).0),
            ping_tx: Arc::new(channel::<PingUpdate>(16).0),
            block_tx: Arc::new(channel::<BlockUpdate>(100).0),
            break_tx: Arc::new(channel::<BreakAnimation>(100).0),
            anim_tx: Arc::new(channel::<AnimationUpdate>(100).0),
            meta_tx: Arc::new(channel::<EntityMetadata>(100).0),
            dmg_tx: Arc::new(channel::<DamageEvent>(100).0),
            item_tx: Arc::new(channel::<ItemDrop>(1000).0),
            despawn_tx: Arc::new(channel::<DespawnEntity>(50).0),
            pickup_tx: Arc::new(channel::<ItemPickup>(100).0),
            time_tx: Arc::new(channel::<TimeUpdate>(1).0),
            weather_tx: Arc::new(channel::<WeatherState>(1).0),
            tick_tx: Arc::new(channel(5).0),
            status_tx: Arc::new(channel::<EntityStatusUpdate>(100).0),
            equip_tx: Arc::new(channel::<EquipmentUpdate>(100).0),
            sound_tx: Arc::new(channel::<SoundEffect>(100).0),
            shutdown_tx: Arc::new(channel::<()>(1).0),
            particle_tx: Arc::new(channel::<ParticleEffect>(100).0),
            projectile_spawn_tx: Arc::new(channel::<ProjectileSpawn>(100).0),
            projectile_move_tx: Arc::new(channel::<ProjectileMove>(200).0),
            splash_effect_tx: Arc::new(channel::<SplashEffect>(100).0),
            xp_orb_spawn_tx: Arc::new(channel::<XpOrbSpawn>(100).0),
            xp_orb_move_tx: Arc::new(channel::<XpOrbMove>(200).0),
            xp_pickup_tx: Arc::new(channel::<XpPickup>(100).0),
            bed_tx: Arc::new(channel::<BedUpdate>(50).0),
            wake_tx: Arc::new(channel::<()>(4).0),
            private_msg_tx: Arc::new(channel::<PrivateMessage>(50).0),
            teleport_rq_tx: Arc::new(channel::<TeleportRequest>(5).0),
            kick_rq_tx: Arc::new(channel::<KickRequest>(5).0),
            sign_update_tx: Arc::new(channel::<SignUpdate>(5).0),
            velocity_broadcast_tx: Arc::new(channel::<EntityVelocityUpdate>(100).0),
            chest_anim_tx: Arc::new(channel::<ChestAnimation>(30).0),
            furnace_update_tx: Arc::new(channel::<(i32, i32, i32)>(100).0),
            difficulty_tx: Arc::new(channel::<u8>(4).0),
        }
    }
}

pub fn world_path(name: &str) -> std::io::Result<&Path> {
    let world_path = Path::new(name);

    if name.ends_with('/')
        || name.ends_with('\\')
        || world_path.components().count() != 1
        || !matches!(world_path.components().next(), Some(Component::Normal(_)))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "Invalid world name {:?}: world_name must be a single directory name",
                name
            ),
        ));
    }
    Ok(world_path)
}

pub async fn register_commands(
    dispatcher: &Arc<CommandDispatcher>,
    channels: &Channels,
    player_registry: &Arc<PlayerRegistry>,
    ops: &Arc<RwLock<OpsFile>>,
    whitelist: &Arc<RwLock<WhitelistFile>>,
    resource_monitor: &Arc<ResourceMonitor>,
    spawn_point: &Arc<RwLock<(f64, f64, f64, f32, f32)>>,
    world_dir: &Arc<PathBuf>,
    world_time: &Arc<AtomicI64>,
) {
    dispatcher.register(list::version::command()).await;
    dispatcher
        .register(list::player_list::command(player_registry.clone()))
        .await;
    dispatcher
        .register(list::gamemode::command(
            player_registry.clone(),
            channels.gm_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::kill::command(
            player_registry.clone(),
            channels.dmg_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::op::command(player_registry.clone(), ops.clone()))
        .await;
    dispatcher
        .register(list::deop::command(player_registry.clone(), ops.clone()))
        .await;
    dispatcher
        .register(list::whitelist::command(
            player_registry.clone(),
            whitelist.clone(),
        ))
        .await;
    dispatcher.register(list::say::command()).await;
    dispatcher
        .register(list::msg::command(
            player_registry.clone(),
            channels.private_msg_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::reply::command(
            player_registry.clone(),
            channels.private_msg_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::usage::command(resource_monitor.clone()))
        .await;
    dispatcher
        .register(
            list::setworldspawn::command(
                player_registry.clone(),
                spawn_point.clone(),
                world_dir.clone(),
            )
            .await,
        )
        .await;
    dispatcher
        .register(list::teleport::command(
            player_registry.clone(),
            channels.teleport_rq_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::kick::command(
            player_registry.clone(),
            channels.kick_rq_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::ping::command(player_registry.clone()))
        .await;
    dispatcher
        .register(list::time::command(world_time.clone()))
        .await;
    dispatcher
        .register(list::difficulty::command(channels.difficulty_tx.clone()))
        .await;
}

#[cfg(test)]
mod tests {
    use super::world_path;
    use std::path::Path;

    #[test]
    fn valid_world_name() {
        assert_eq!(world_path("world").unwrap(), Path::new("world"));
    }

    #[test]
    fn invalid_world_name() {
        assert!(world_path("world/").is_err());
        assert!(world_path("world\\").is_err());
        assert!(world_path("/world").is_err());
        assert!(world_path("../world").is_err());
        assert!(world_path("foo/bar").is_err());
        assert!(world_path("./world").is_err());
        assert!(world_path("..").is_err());
        assert!(world_path(".").is_err());
        assert!(world_path("").is_err());
    }
}
