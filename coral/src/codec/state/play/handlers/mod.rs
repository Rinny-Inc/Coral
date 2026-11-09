use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use coral::Channels;
use coral_command::CommandDispatcher;
use coral_config::Config;
use coral_server::{
    entity_tracker::EntityTracker,
    experience::XpOrb,
    items::{ItemRegistry, drops::DroppedItems},
    player::registry::PlayerRegistry,
    projectile::Projectile,
    tile_entities::TileEntityStore,
};
use coral_world::{blocks::registry::BlockRegistry, spawn::SpawnPos, world::World};
use tokio::sync::RwLock;

pub(super) mod chat;
pub(super) mod entity;
pub(super) mod inventory;
pub(super) mod movement;
pub(super) mod session;
pub(super) mod world;

/// What the select loop does once a handler returns
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Flow {
    /// Handled; wait for the next packet
    Continue,
    /// Drop the connection
    Disconnect,
}

pub(super) struct PlayCtx<'a> {
    pub player_registry: &'a Arc<PlayerRegistry>,
    pub item_registry: &'a Arc<ItemRegistry>,
    pub config: &'a Arc<Config>,
    pub channels: &'a Channels,
    pub projectiles: &'a Arc<RwLock<Vec<Projectile>>>,
    pub tile_entities: &'a Arc<RwLock<TileEntityStore>>,
    pub entity_tracker: &'a Arc<RwLock<EntityTracker>>,
    pub item_positions: &'a Arc<RwLock<DroppedItems>>,
    pub item_spawn_times: &'a Arc<RwLock<HashMap<i32, Instant>>>,
    pub dispatcher: &'a Arc<CommandDispatcher>,
    pub world: &'a Arc<World>,
    pub spawn_point: &'a Arc<RwLock<SpawnPos>>,
    pub server_loaded_chunks: &'a Arc<RwLock<HashSet<(i32, i32)>>>,
    pub xp_orbs: &'a Arc<RwLock<Vec<XpOrb>>>,
    pub block_registry: &'a Arc<BlockRegistry>,
    pub fluid_queue: &'a Arc<RwLock<FluidQueue>>,
    pub client_protocol: i32,
}

macro_rules! flow {
    ($call:expr) => {
        match $call {
            crate::codec::state::play::handlers::Flow::Continue => continue,
            crate::codec::state::play::handlers::Flow::Disconnect => break,
        }
    };
}
pub(super) use flow;

use crate::fluid_sim::FluidQueue;
