use std::collections::HashMap;
use std::sync::Arc;

use coral_protocol::packets::play::{
    chat::builder::ChatBuilder,
    entity::TileEntity,
    inventory::{ClickWindow, OpenWindow, SetSlot, WindowItems},
};
use coral_types::ChestAnimation;
use tokio::{
    net::TcpStream,
    sync::{RwLock, broadcast::Sender},
};
use tokio_util::codec::Framed;

use crate::codec::{
    Codec, PlayerState, WindowType, send_packet, state::play::tile_entity_window::TileEntityWindow,
};

pub async fn open_tile_entity_window(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    pos: (i32, i32, i32),
    tile_entities: &Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
    chest_anim_tx: &Arc<Sender<ChestAnimation>>,
    sound_tx: &Arc<Sender<crate::SoundEffect>>,
) -> bool {
    state.window_id_counter = state.window_id_counter.wrapping_add(1);
    if state.window_id_counter == 0 {
        state.window_id_counter = 1;
    }
    let window_id = state.window_id_counter;

    let open_data = {
        let mut storage = tile_entities.write().await;
        let tile = storage.entry(pos).or_insert_with(|| TileEntity::Chest {
            items: vec![None; 27],
            viewers: 0,
        });

        if !tile.has_window() {
            return false;
        }
        tile.open().await
    };

    let Some(data) = open_data else { return false };

    let window_type = (data.window_type_ctor)(window_id, pos);

    send_packet(
        framed,
        OpenWindow {
            window_id,
            window_type: window_type.clone(),
            title: ChatBuilder::new(data.title),
            slot_count: data.slot_count,
        },
    )
    .await;

    let mut slots = data.container_slots;
    for internal in 9..36 {
        match &state.inventory.slots[internal] {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }
    for internal in 0..9 {
        match &state.inventory.slots[internal] {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }

    send_packet(framed, WindowItems { window_id, slots }).await;
    state.open_window = Some(window_type);

    if let Some(TileEntity::Chest { viewers, .. }) = tile_entities.read().await.get(&pos) {
        chest_anim_tx.send((pos.0, pos.1, pos.2, *viewers)).ok();
        sound_tx
            .send((
                "random.chestopen".to_string(),
                pos.0 as f64 + 0.5,
                pos.1 as f64 + 0.5,
                pos.2 as f64 + 0.5,
                0.5,
                63,
            ))
            .ok();
    }

    true
}

pub async fn handle_tile_entity_click(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    click: &ClickWindow,
    tile_entities: &Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
) -> bool {
    let Some(open) = state.open_window.clone() else {
        return false;
    };
    if click.window_id != open.window_id() {
        return false;
    }

    let pos = match &open {
        WindowType::Chest { pos, .. } => *pos,
        WindowType::Furnace { pos, .. } => *pos,
        _ => return false,
    };

    let updated_slots = {
        let mut storage = tile_entities.write().await;
        let Some(tile) = storage.get_mut(&pos) else {
            return false;
        };
        tile.click(state, click).await
    };

    let Some(container_slots) = updated_slots else {
        return false;
    };

    // resend the full window
    let mut slots = container_slots;
    for internal in 9..36 {
        match &state.inventory.slots[internal] {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }
    for internal in 0..9 {
        match &state.inventory.slots[internal] {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }
    send_packet(
        framed,
        WindowItems {
            window_id: open.window_id(),
            slots,
        },
    )
    .await;

    send_packet(
        framed,
        SetSlot {
            window_id: -1,
            slot: -1,
            item_id: state.cursor_item.as_ref().map(|s| s.item_id).unwrap_or(-1),
            count: state.cursor_item.as_ref().map(|s| s.count).unwrap_or(0),
            metadata: state.cursor_item.as_ref().map(|s| s.metadata).unwrap_or(0),
        },
    )
    .await;

    true
}

pub async fn close_tile_entity_window(
    state: &mut PlayerState,
    tile_entities: &Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
    chest_anim_tx: &Arc<Sender<ChestAnimation>>,
    sound_tx: &Arc<Sender<crate::SoundEffect>>,
) {
    let Some(open) = state.open_window.take() else {
        return;
    };

    let pos = match &open {
        WindowType::Chest { pos, .. } => *pos,
        _ => return,
    };

    let viewers = {
        let mut storage = tile_entities.write().await;
        let Some(tile) = storage.get_mut(&pos) else {
            return;
        };
        tile.close().await
    };

    chest_anim_tx.send((pos.0, pos.1, pos.2, viewers)).ok();

    if viewers == 0 {
        sound_tx
            .send((
                "random.chestclosed".to_string(),
                pos.0 as f64 + 0.5,
                pos.1 as f64 + 0.5,
                pos.2 as f64 + 0.5,
                0.5,
                63,
            ))
            .ok();
    }
}
