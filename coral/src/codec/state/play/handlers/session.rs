use coral_protocol::packets::play::{
    ClientSettings, PlayerAbilities, PluginMessage, ResourcePackResult, ResourcePackStatus,
    block::HeldItemChange,
    chat::builder::ChatBuilder,
    entity::{MetadataValue, TileEntity, UpdateSign},
    inventory::Inventory,
    keepalive::KeepAlive,
};
use coral_server::items::ids::EMPTY_HAND;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::codec::{
    Codec, PlayerState, kick, send_packet,
    state::play::{
        drop_item_in_world,
        handlers::{Flow, PlayCtx},
        window_ops::close_tile_entity_window,
    },
};

pub(in crate::codec::state::play) async fn keep_alive(
    ka: KeepAlive,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    if let Some((sent_id, sent_time)) = state.last_sent_keep_alive
        && ka.id == sent_id
    {
        state.last_sent_keep_alive = None;
        let previous = state.latency_ms.0;
        let measured = sent_time.elapsed().as_millis() as i32;
        state.latency_ms = (measured, previous);
        ctx.player_registry
            .update_latency(&state.uuid, measured)
            .await;
        if ping_to_bar(measured) == ping_to_bar(previous) {
            return Flow::Continue;
        }
        ctx.channels.ping_tx.send((state.uuid, measured)).ok();
    }
    Flow::Continue
}

// ;) nPaper
fn ping_to_bar(ping: i32) -> u8 {
    if ping <= 0 {
        return 5;
    }
    if ping < 150 {
        return 0;
    }
    if ping < 300 {
        return 1;
    }
    if ping < 600 {
        return 2;
    }
    if ping < 1000 {
        return 3;
    }
    4
}

pub(in crate::codec::state::play) async fn held_item_change(
    held: HeldItemChange,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    let slot = held.slot.clamp(0, 8) as u8;
    state.held_slot = slot;

    let internal_idx = Inventory::packet_to_internal(36 + slot as i16).unwrap_or(slot as usize);

    state.held_item = ctx.item_registry.resolve(
        state.inventory.slots[internal_idx]
            .as_ref()
            .map(|s| s.item_id)
            .unwrap_or(EMPTY_HAND),
    );

    ctx.player_registry
        .update_held_slot(&state.uuid, slot)
        .await;
    ctx.player_registry
        .update_held_item(&state.uuid, state.held_item.id())
        .await;

    state.send_held_equip(&ctx.channels.equip_tx);

    state.eating = None;
    state.bow_charging = None;
    state
        .try_retract_fishing_hook(ctx.projectiles, &ctx.channels.despawn_tx)
        .await;
    Flow::Continue
}

pub(in crate::codec::state::play) async fn close_window(
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    if let Some(cursor) = state.cursor_item.take()
        && let Some(dropped) = state.inventory.insert_itemstack(cursor)
    {
        drop_item_in_world(
            dropped,
            state.entity_id,
            ctx.player_registry,
            ctx.channels,
            ctx.item_spawn_times,
            ctx.entity_tracker,
            ctx.item_positions,
            ctx.config.tracking.item,
        )
        .await;
    }
    close_tile_entity_window(
        state,
        ctx.tile_entities,
        &ctx.channels.chest_anim_tx,
        &ctx.channels.sound_tx,
    )
    .await;
    Flow::Continue
}

pub(in crate::codec::state::play) async fn resource_pack_status(
    status: ResourcePackStatus,
    framed: &mut Framed<TcpStream, Codec>,
    ctx: &PlayCtx<'_>,
) -> Flow {
    match status.result {
        ResourcePackResult::Loaded | ResourcePackResult::Accepted => {}
        ResourcePackResult::Decline => {
            if !ctx.config.resource_pack.forced {
                return Flow::Continue;
            }
            kick(framed, "You must accept the ressource pack to play!").await;
            return Flow::Disconnect;
        }
        ResourcePackResult::Failed => {
            if !ctx.config.resource_pack.forced {
                return Flow::Continue;
            }
            kick(framed, "Resource pack download failed!").await;
            return Flow::Disconnect;
        }
    }
    Flow::Continue
}

pub(in crate::codec::state::play) fn player_abilities(
    abilities: PlayerAbilities,
    state: &mut PlayerState,
) -> Flow {
    let is_flying = (abilities.flags & 0x02) != 0;
    state.is_flying = is_flying;
    Flow::Continue
}

pub(in crate::codec::state::play) async fn update_sign(
    sign: UpdateSign,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    let pos = (sign.x, sign.y, sign.z);

    if state.editing_sign != Some(pos) {
        return Flow::Continue;
    }
    state.editing_sign = None;

    let sanitize =
        |s: &str| -> String { s.chars().filter(|c| *c != '\u{00a7}').take(16).collect() };

    let lines = [
        sanitize(&sign.lines[0]),
        sanitize(&sign.lines[1]),
        sanitize(&sign.lines[2]),
        sanitize(&sign.lines[3]),
    ];

    let json_lines = [
        ChatBuilder::plain_json(&lines[0]),
        ChatBuilder::plain_json(&lines[1]),
        ChatBuilder::plain_json(&lines[2]),
        ChatBuilder::plain_json(&lines[3]),
    ];

    ctx.tile_entities
        .write()
        .await
        .insert(pos, TileEntity::Sign { lines });
    ctx.channels
        .sign_update_tx
        .send((sign.x, sign.y, sign.z, json_lines))
        .ok();
    Flow::Continue
}

pub(in crate::codec::state::play) async fn client_settings(
    settings: ClientSettings,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    state.skin_parts = settings.skin_parts;
    ctx.player_registry
        .update_skin_parts(&state.uuid, settings.skin_parts)
        .await;
    state
        .watcher
        .set(10, MetadataValue::Byte(settings.skin_parts));
    Flow::Continue
}

pub(in crate::codec::state::play) async fn plugin_message(
    plugin: PluginMessage,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
) -> Flow {
    if plugin.channel == "MC|Brand" {
        if let Ok(brand) = String::from_utf8(plugin.data.clone()) {
            let brand = brand.trim_matches('\0').to_string();
            state.client_brand = Some(brand);
        }
        send_packet(framed, PluginMessage::brand("Coral")).await;
    }
    Flow::Continue
}
