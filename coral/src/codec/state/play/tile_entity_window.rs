use coral_protocol::packets::play::{
    entity::TileEntity,
    inventory::{ClickWindow, ItemStack},
};
use coral_server::smelting::fuel_burn_ticks;

use crate::codec::{PlayerState, WindowType};

pub struct WindowOpenData {
    pub window_type_ctor: fn(u8, (i32, i32, i32)) -> WindowType,
    pub slot_count: u8,
    pub title: &'static str,
    pub container_slots: Vec<(i16, u8, i16)>,
}

pub trait TileEntityWindow {
    fn has_window(&self) -> bool;

    async fn open(&mut self) -> Option<WindowOpenData>;

    async fn click(
        &mut self,
        state: &mut PlayerState,
        click: &ClickWindow,
    ) -> Option<Vec<(i16, u8, i16)>>;

    async fn close(&mut self) -> u8;
}

impl TileEntityWindow for TileEntity {
    fn has_window(&self) -> bool {
        matches!(self, TileEntity::Chest { .. } | TileEntity::Furnace { .. })
    }

    async fn open(&mut self) -> Option<WindowOpenData> {
        match self {
            TileEntity::Chest { items, viewers } => {
                *viewers += 1;
                let container_slots = items
                    .iter()
                    .take(27)
                    .map(|item| match item {
                        Some(s) => (s.item_id, s.count, s.metadata),
                        None => (-1, 0, 0),
                    })
                    .collect();

                Some(WindowOpenData {
                    window_type_ctor: |window_id, pos| WindowType::Chest { window_id, pos },
                    slot_count: 27,
                    title: "Chest",
                    container_slots,
                })
            }
            TileEntity::Furnace {
                input,
                fuel,
                output,
                viewers,
                ..
            } => {
                *viewers += 1;
                let container_slots = vec![slot_tuple(input), slot_tuple(fuel), slot_tuple(output)];
                Some(WindowOpenData {
                    window_type_ctor: |window_id, pos| WindowType::Furnace { window_id, pos },
                    slot_count: 3,
                    title: "Furnace",
                    container_slots,
                })
            }
            _ => None,
        }
    }

    async fn click(
        &mut self,
        state: &mut PlayerState,
        click: &ClickWindow,
    ) -> Option<Vec<(i16, u8, i16)>> {
        match self {
            super::TileEntity::Chest { items, .. } => {
                fn resolve(slot: i16) -> Option<(bool, usize)> {
                    match slot {
                        0..=26 => Some((true, slot as usize)),
                        27..=53 => Some((false, (slot - 27 + 9) as usize)),
                        54..=62 => Some((false, (slot - 54) as usize)),
                        _ => None,
                    }
                }

                match click.mode {
                    0 => {
                        let (is_chest, idx) = resolve(click.slot)?;
                        let slot_item = if is_chest {
                            items[idx].take()
                        } else {
                            state.inventory.slots[idx].take()
                        };
                        let cursor = state.cursor_item.take();
                        if is_chest {
                            items[idx] = cursor;
                        } else {
                            state.inventory.slots[idx] = cursor;
                        }
                        state.cursor_item = slot_item;
                    }
                    1 => {
                        let (is_chest, idx) = resolve(click.slot)?;
                        let moving = if is_chest {
                            items[idx].take()
                        } else {
                            state.inventory.slots[idx].take()
                        };
                        if let Some(stack) = moving {
                            if is_chest {
                                let leftover = insert_into_inventory(&mut state.inventory, stack);
                                items[idx] = leftover;
                            } else {
                                let leftover = insert_into_chest(items, stack);
                                state.inventory.slots[idx] = leftover;
                            }
                        }
                    }
                    _ => {}
                }

                Some(
                    items
                        .iter()
                        .take(27)
                        .map(|item| match item {
                            Some(s) => (s.item_id, s.count, s.metadata),
                            None => (-1, 0, 0),
                        })
                        .collect(),
                )
            }
            TileEntity::Furnace {
                input,
                fuel,
                output,
                ..
            } => {
                fn resolve(slot: i16) -> Option<(u8, usize)> {
                    match slot {
                        0 => Some((0, 0)),                            // input
                        1 => Some((0, 1)),                            // fueld
                        2 => Some((0, 2)),                            // output
                        3..=29 => Some((1, (slot - 3 + 9) as usize)), // main inv
                        30..=38 => Some((1, (slot - 30) as usize)),   // hotbar
                        _ => None,
                    }
                }
                let (region, idx) = resolve(click.slot)?;

                if click.mode == 0 {
                    let slot_item = if region == 0 {
                        match idx {
                            0 => input.take(),
                            1 => fuel.take(),
                            2 => output.take(),
                            _ => None,
                        }
                    } else {
                        state.inventory.slots[idx].take()
                    };
                    let cursor = state.cursor_item.take();
                    if region == 0 {
                        match idx {
                            0 => *input = cursor,
                            1 => *fuel = cursor,
                            2 => {
                                state.cursor_item = cursor;
                                state.cursor_item = slot_item;
                                return Some(vec![
                                    slot_tuple(input),
                                    slot_tuple(fuel),
                                    slot_tuple(output),
                                ]);
                            }
                            _ => {}
                        }
                    } else {
                        state.inventory.slots[idx] = cursor;
                    }
                    state.cursor_item = slot_item;
                } else if click.mode == 1 {
                    // shift
                    let moving = if region == 0 {
                        match idx {
                            0 => input.take(),
                            1 => fuel.take(),
                            2 => output.take(),
                            _ => None,
                        }
                    } else {
                        state.inventory.slots[idx].take()
                    };
                    if let Some(stack) = moving {
                        if region == 0 {
                            let leftover = insert_into_inventory(&mut state.inventory, stack);
                            match idx {
                                0 => *input = leftover,
                                1 => *fuel = leftover,
                                2 => *output = leftover,
                                _ => {}
                            }
                        } else {
                            if fuel_burn_ticks(stack.item_id).is_some() && fuel.is_none() {
                                *fuel = Some(stack);
                            } else if input.is_none() {
                                *input = Some(stack);
                            } else {
                                state.inventory.slots[idx] = Some(stack);
                            }
                        }
                    }
                }

                Some(vec![
                    slot_tuple(input),
                    slot_tuple(fuel),
                    slot_tuple(output),
                ])
            }
            _ => None,
        }
    }

    async fn close(&mut self) -> u8 {
        match self {
            TileEntity::Chest { viewers, .. } => {
                *viewers = viewers.saturating_sub(1);
                *viewers
            }
            TileEntity::Furnace { viewers, .. } => {
                *viewers = viewers.saturating_sub(1);
                *viewers
            }
            _ => 0,
        }
    }
}

fn slot_tuple(item: &Option<ItemStack>) -> (i16, u8, i16) {
    match item {
        Some(s) => (s.item_id, s.count, s.metadata),
        None => (-1, 0, 0),
    }
}

fn insert_into_inventory(
    inv: &mut coral_protocol::packets::play::inventory::Inventory,
    mut stack: ItemStack,
) -> Option<ItemStack> {
    for slot in inv.slots.iter_mut().take(36).flatten() {
        if slot.item_id == stack.item_id && slot.metadata == stack.metadata && slot.count < 64 {
            let space = 64 - slot.count;
            let move_n = space.min(stack.count);
            slot.count += move_n;
            stack.count -= move_n;
            if stack.count == 0 {
                return None;
            }
        }
    }
    for slot in inv.slots.iter_mut().take(36) {
        if slot.is_none() {
            *slot = Some(stack);
            return None;
        }
    }
    Some(stack)
}

fn insert_into_chest(chest: &mut [Option<ItemStack>], mut stack: ItemStack) -> Option<ItemStack> {
    for existing in chest.iter_mut().flatten() {
        if existing.item_id == stack.item_id
            && existing.metadata == stack.metadata
            && existing.count < 64
        {
            let space = 64 - existing.count;
            let move_n = space.min(stack.count);
            existing.count += move_n;
            stack.count -= move_n;
            if stack.count == 0 {
                return None;
            }
        }
    }
    for existing in chest.iter_mut() {
        if existing.is_none() {
            *existing = Some(stack);
            return None;
        }
    }
    Some(stack)
}
