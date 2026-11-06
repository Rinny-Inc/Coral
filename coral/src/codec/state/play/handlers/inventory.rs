use coral_protocol::packets::play::inventory::{
    ClickWindow, ConfirmTransaction, CreativeInventoryAction, Inventory, ItemStack,
};
use coral_server::items::ids::EMPTY_HAND;
use coral_types::GameMode;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::codec::{
    Codec, PlayerState, send_packet,
    state::play::{
        PlayCtx, handlers::Flow, inventory_click, spawn_dropped_item,
        window_ops::handle_tile_entity_click,
    },
};

pub(in crate::codec::state::play) async fn creative_inventory_action(
    creative: CreativeInventoryAction,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    if state.gamemode != GameMode::Creative {
        return Flow::Continue;
    }
    let internal = Inventory::packet_to_internal(creative.slot);
    if let Some(idx) = internal {
        if creative.item_id == EMPTY_HAND {
            state.inventory.slots[idx] = None;
        } else {
            state.inventory.slots[idx] = Some(ItemStack {
                item_id: creative.item_id,
                count: creative.item_count,
                metadata: creative.item_damage,
                durability: 0,
            });
        }
        if idx < 9 && idx == state.held_slot as usize {
            state
                .sync_held_item_registry(
                    creative.item_id,
                    ctx.player_registry,
                    ctx.item_registry,
                    ctx.channels,
                )
                .await;
        }
        if Inventory::ARMOR_SLOTS.contains(&idx) {
            state
                .sync_armor(ctx.player_registry, &ctx.channels.equip_tx)
                .await;
        }
    }
    Flow::Continue
}

pub(in crate::codec::state::play) async fn click_window(
    click: ClickWindow,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    send_packet(
        framed,
        ConfirmTransaction {
            window_id: click.window_id,
            action_number: click.action_number,
            accepted: true,
        },
    )
    .await;

    if handle_tile_entity_click(framed, state, &click, ctx.tile_entities).await {
        return Flow::Continue;
    }

    let mut dropped = Vec::new();
    let outcome = inventory_click::apply_click(
        &mut state.inventory.slots,
        &mut state.cursor_item,
        &click,
        &mut dropped,
    );

    for stack in dropped {
        if let Some(p) = ctx.player_registry.get(&state.uuid).await {
            spawn_dropped_item(
                &p,
                stack,
                ctx.channels,
                ctx.item_spawn_times,
                ctx.entity_tracker,
                ctx.item_positions,
                ctx.config.tracking.item,
            )
            .await;
        }
    }
    match outcome {
        inventory_click::ClickOutcome::AppliedArmorChanged => {
            state
                .sync_armor(ctx.player_registry, &ctx.channels.equip_tx)
                .await;
        }
        inventory_click::ClickOutcome::Applied => {}
        inventory_click::ClickOutcome::Resync => {
            state.send_full_inventory(framed).await;
        }
    }
    if Inventory::packet_to_internal(click.slot) == Some(state.held_slot as usize) {
        let held = state.inventory.slots[state.held_slot as usize]
            .as_ref()
            .map(|s| s.item_id)
            .unwrap_or(EMPTY_HAND);
        state
            .sync_held_item_registry(held, ctx.player_registry, ctx.item_registry, ctx.channels)
            .await;
    }
    Flow::Continue
}
