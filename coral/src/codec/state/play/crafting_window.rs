use coral_protocol::packets::play::{
    chat::builder::ChatBuilder,
    inventory::{ClickWindow, Inventory, ItemStack, OpenWindow, SetSlot, WindowItems, WindowType},
};
use coral_server::{
    crafting::{CraftingGrid, OutputClick, RecipeRegistry, craft_once},
    items::ItemRegistry,
};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::codec::{Codec, PlayerState, send_packet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftLayout {
    Player,
    Bench,
}

/// Where a packet slot falls in a window that has a crafting grid
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CraftSlot {
    /// The result slot
    Output,
    /// A grid cell indexed row-major within the grid's own width
    Grid(usize),
    /// A slot of the player's 44-entry inventory
    Inventory(usize),
}
impl CraftLayout {
    /// How many cells the grid has which is also where the inventory starts
    pub fn grid_cells(&self) -> usize {
        match self {
            Self::Player => 4,
            Self::Bench => 9,
        }
    }

    /// Resolve a packet slot into what it actually addresses
    ///
    /// Returns `None` for a slot this window does not have
    pub fn resolve(&self, slot: i16) -> Option<CraftSlot> {
        if slot == 0 {
            return Some(CraftSlot::Output);
        }
        let cells = self.grid_cells() as i16;
        if (1..=cells).contains(&slot) {
            return Some(CraftSlot::Grid((slot - 1) as usize));
        }

        match self {
            Self::Player => Inventory::packet_to_internal(slot).map(CraftSlot::Inventory),
            Self::Bench => match slot {
                10..=36 => Some(CraftSlot::Inventory((slot - 10 + 9) as usize)),
                37..=45 => Some(CraftSlot::Inventory((slot - 37) as usize)),
                _ => None,
            },
        }
    }
}

fn same_item(a: &ItemStack, b: &ItemStack) -> bool {
    a.item_id == b.item_id && a.metadata == b.metadata && a.durability == b.durability
}

/// The pick-up place and merge behavior of one ordinary slot
fn swap_or_merge(
    slot: &mut Option<ItemStack>,
    cursor: &mut Option<ItemStack>,
    max_stack: u8,
    whole: bool,
) {
    match (cursor.take(), slot.take()) {
        (None, Some(mut in_slot)) => {
            if whole {
                *cursor = Some(in_slot);
            } else {
                let taken = in_slot.count.div_ceil(2);
                in_slot.count -= taken;
                *cursor = Some(ItemStack {
                    count: taken,
                    ..in_slot
                });
                if in_slot.count > 0 {
                    *slot = Some(in_slot);
                }
            }
        }
        (Some(mut held), None) => {
            if whole {
                *slot = Some(held);
            } else {
                *slot = Some(ItemStack { count: 1, ..held });
                held.count -= 1;
                if held.count > 0 {
                    *cursor = Some(held);
                }
            }
        }
        (Some(mut held), Some(mut in_slot)) => {
            if same_item(&held, &in_slot) && in_slot.count < max_stack {
                let room = max_stack - in_slot.count;
                let moved = if whole {
                    room.min(held.count)
                } else {
                    1.min(room)
                };
                in_slot.count += moved;
                held.count -= moved;
                *slot = Some(in_slot);
                if held.count > 0 {
                    *cursor = Some(held);
                }
            } else {
                *slot = Some(held);
                *cursor = Some(in_slot);
            }
        }
        (None, None) => {}
    }
}

/// Which layout if any a click addresses
pub fn layout_for(open: Option<&WindowType>, window_id: u8) -> Option<CraftLayout> {
    match open {
        Some(WindowType::Crafting { window_id: open_id }) => {
            (*open_id == window_id).then_some(CraftLayout::Bench)
        }
        None if window_id == 0 => Some(CraftLayout::Player),
        _ => None,
    }
}

pub struct CraftDeps<'a> {
    pub recipes: &'a RecipeRegistry,
    pub items: &'a ItemRegistry,
}

/// The outcome of a crafting click
pub enum CraftClick {
    /// Not a slot this module owns; let the ordinary inventory path have it
    NotMine,
    /// Handled
    /// The window and cursor have been resent
    Handled,
    /// Handled and these stacks could not fit and must be dropped
    HandledWithOverflow(Vec<ItemStack>),
}

/// Handle a click in a window that has a crafting grid
pub async fn handle_crafting_click(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    click: &ClickWindow,
    deps: &CraftDeps<'_>,
) -> CraftClick {
    let Some(layout) = layout_for(state.open_window.as_ref(), click.window_id) else {
        return CraftClick::NotMine;
    };
    let Some(overflow) = apply_crafting_click(
        &mut state.crafting_grid,
        &mut state.inventory,
        &mut state.cursor_item,
        layout,
        click,
        deps,
    ) else {
        return CraftClick::NotMine;
    };

    send_crafting_window(framed, state, layout, deps.recipes).await;

    if overflow.is_empty() {
        CraftClick::Handled
    } else {
        CraftClick::HandledWithOverflow(overflow)
    }
}

pub fn apply_crafting_click(
    grid: &mut CraftingGrid,
    inventory: &mut Inventory,
    cursor: &mut Option<ItemStack>,
    layout: CraftLayout,
    click: &ClickWindow,
    deps: &CraftDeps<'_>,
) -> Option<Vec<ItemStack>> {
    let target = layout.resolve(click.slot)?;

    if layout == CraftLayout::Player && !matches!(target, CraftSlot::Output | CraftSlot::Grid(_)) {
        return None;
    }

    let stack_limit = |stack: Option<&ItemStack>| {
        stack
            .map(|s| deps.items.resolve(s.item_id).max_stack_size())
            .unwrap_or(64)
    };

    let mut overflow = Vec::new();

    match (target, click.mode, click.button) {
        // Taking the result: one batch onto the cursor
        (CraftSlot::Output, 0, _) => {
            let max_stack = deps
                .recipes
                .result_for(grid)
                .map(|result| deps.items.resolve(result.item_id).max_stack_size())
                .unwrap_or(64);

            if let OutputClick::Crafted(held) =
                craft_once(grid, deps.recipes, cursor.as_ref(), max_stack)
            {
                *cursor = Some(held);
            }
        }
        // Shift-clicking the result crats until something runs out
        (CraftSlot::Output, 1, _) => {
            let limit = grid.max_crafts();
            for _ in 0..limit {
                let Some(result) = deps.recipes.result_for(grid) else {
                    break;
                };
                let max_stack = deps.items.resolve(result.item_id).max_stack_size();

                match craft_once(grid, deps.recipes, None, max_stack) {
                    OutputClick::Crafted(batch) => {
                        if let Some(left) = inventory.insert_itemstack(batch) {
                            overflow.push(left);
                            break;
                        }
                    }
                    OutputClick::Rejected => break,
                }
            }
        }
        // A grid cell behaves like any other slot
        (CraftSlot::Grid(cell), 0, button) => {
            let max_stack = match grid.get(cell) {
                Some(in_slot) => stack_limit(Some(in_slot)),
                None => stack_limit(cursor.as_ref()),
            };

            let mut slot = grid.get(cell).cloned();
            swap_or_merge(&mut slot, cursor, max_stack, button == 0);
            grid.set(cell, slot);
        }
        // Shift-clicking a grid cell sends it to the inventory
        (CraftSlot::Grid(cell), 1, _) => {
            if let Some(stack) = grid.get(cell).cloned() {
                match inventory.insert_itemstack(stack) {
                    Some(left) => grid.set(cell, Some(left)),
                    None => grid.set(cell, None),
                }
            }
        }
        // Bench only: the inventory half of the window
        (CraftSlot::Inventory(index), 0, button) => {
            let max_stack = match inventory.slots[index].as_ref() {
                Some(in_slot) => stack_limit(Some(in_slot)),
                None => stack_limit(cursor.as_ref()),
            };

            let mut slot = inventory.slots[index].take();
            swap_or_merge(&mut slot, cursor, max_stack, button == 0);
            inventory.slots[index] = slot;
        }
        // Shift-clicking the inventory half moves the stack between the hotbar and the main inventory
        (CraftSlot::Inventory(index), 1, _) => {
            quick_move_within_inventory(inventory, index);
        }
        _ => {}
    }

    Some(overflow)
}

fn quick_move_within_inventory(inventory: &mut Inventory, index: usize) {
    const HOTBAR: std::ops::Range<usize> = 0..9;
    const MAIN: std::ops::Range<usize> = 9..36;

    let Some(mut moving) = inventory.slots[index].take() else {
        return;
    };

    let destination = if HOTBAR.contains(&index) {
        MAIN
    } else if MAIN.contains(&index) {
        HOTBAR
    } else {
        inventory.slots[index] = Some(moving);
        return;
    };

    for slot in destination.clone() {
        let Some(existing) = inventory.slots[slot].as_mut() else {
            continue;
        };
        if !same_item(existing, &moving) || existing.count >= 64 {
            continue;
        }
        let room = 64 - existing.count;
        let moved = room.min(moving.count);
        existing.count += moved;
        moving.count -= moved;
        if moving.count == 0 {
            return;
        }
    }

    for slot in destination {
        if inventory.slots[slot].is_none() {
            inventory.slots[slot] = Some(moving);
            return;
        }
    }

    inventory.slots[index] = Some(moving);
}

fn slot_tuple(stack: Option<&ItemStack>) -> (i16, u8, i16) {
    match stack {
        Some(s) => (s.item_id, s.count, s.metadata),
        None => (-1, 0, 0),
    }
}

/// Resend the whole window result slot included
pub async fn send_crafting_window(
    framed: &mut Framed<TcpStream, Codec>,
    state: &PlayerState,
    layout: CraftLayout,
    recipes: &RecipeRegistry,
) {
    let result = recipes.result_for(&state.crafting_grid);
    let mut slots = vec![slot_tuple(result.as_ref())];

    for cell in 0..layout.grid_cells() {
        slots.push(slot_tuple(state.crafting_grid.get(cell)));
    }

    match layout {
        CraftLayout::Player => {
            // Armour then main then hotbar
            for internal in Inventory::ARMOR_SLOTS {
                slots.push(slot_tuple(state.inventory.slots[internal].as_ref()));
            }
            for internal in state.inventory.slots.iter().take(36).skip(9) {
                slots.push(slot_tuple(internal.as_ref()));
            }
            for internal in state.inventory.slots.iter().take(9) {
                slots.push(slot_tuple(internal.as_ref()));
            }
        }
        CraftLayout::Bench => {
            for internal in state.inventory.slots.iter().take(36).skip(9) {
                slots.push(slot_tuple(internal.as_ref()));
            }
            for internal in state.inventory.slots.iter().take(9) {
                slots.push(slot_tuple(internal.as_ref()));
            }
        }
    }

    let window_id = match layout {
        CraftLayout::Player => 0,
        CraftLayout::Bench => state
            .open_window
            .as_ref()
            .map(|w| w.window_id())
            .unwrap_or(0),
    };

    send_packet(framed, WindowItems { window_id, slots }).await;

    let cursor = state.cursor_item.as_ref();
    send_packet(
        framed,
        SetSlot {
            window_id: -1,
            slot: -1,
            item_id: cursor.map(|s| s.item_id).unwrap_or(-1),
            count: cursor.map(|s| s.count).unwrap_or(0),
            metadata: cursor.map(|s| s.metadata).unwrap_or(0),
        },
    )
    .await
}

pub async fn open_crafting_window(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    recipes: &RecipeRegistry,
) -> bool {
    state.window_id_counter = state.window_id_counter.wrapping_add(1);
    if state.window_id_counter == 0 {
        state.window_id_counter = 1;
    }
    let window_id = state.window_id_counter;

    send_packet(
        framed,
        OpenWindow {
            window_id,
            window_type: WindowType::Crafting { window_id },
            title: ChatBuilder::new("Crafting"),
            slot_count: 10,
        },
    )
    .await;

    state.open_window = Some(WindowType::Crafting { window_id });
    state.crafting_grid = CraftingGrid::bench();

    send_crafting_window(framed, state, CraftLayout::Bench, recipes).await;
    true
}

/// Empty the grid back into the inventory when a window closes
pub fn reclaim_grid(state: &mut PlayerState) -> Vec<ItemStack> {
    let mut overflow = Vec::new();

    for cell in 0..state.crafting_grid.len() {
        if let Some(stack) = state.crafting_grid.get(cell).cloned()
            && let Some(left) = state.inventory.insert_itemstack(stack)
        {
            overflow.push(left);
        }
    }

    state.crafting_grid = CraftingGrid::player();
    overflow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_layouts_number_the_inventory_differently() {
        assert_eq!(
            CraftLayout::Player.resolve(10),
            Some(CraftSlot::Inventory(10))
        );
        assert_eq!(
            CraftLayout::Bench.resolve(10),
            Some(CraftSlot::Inventory(9))
        );
    }

    #[test]
    fn slot_zero_is_always_the_result() {
        for layout in [CraftLayout::Player, CraftLayout::Bench] {
            assert_eq!(layout.resolve(0), Some(CraftSlot::Output));
        }
    }

    #[test]
    fn the_player_window_has_four_grid_cells_and_a_bench_has_nine() {
        for slot in 1..=4 {
            assert_eq!(
                CraftLayout::Player.resolve(slot),
                Some(CraftSlot::Grid((slot - 1) as usize))
            );
        }
        assert_eq!(
            CraftLayout::Player.resolve(5),
            Some(CraftSlot::Inventory(36))
        );

        for slot in 1..=9 {
            assert_eq!(
                CraftLayout::Bench.resolve(slot),
                Some(CraftSlot::Grid((slot - 1) as usize))
            );
        }
    }

    #[test]
    fn each_layout_covers_its_slots_without_collisions() {
        for (layout, last) in [(CraftLayout::Player, 44i16), (CraftLayout::Bench, 45)] {
            let mut seen_grid = vec![];
            let mut seen_inventory = vec![];

            for slot in 0..=last {
                let resolved = layout
                    .resolve(slot)
                    .unwrap_or_else(|| panic!("{layout:?} slot {slot} resolves to nothing"));

                match resolved {
                    CraftSlot::Output => {}
                    CraftSlot::Grid(cell) => {
                        assert!(
                            !seen_grid.contains(&cell),
                            "{layout:?} maps two slots to grid cell {cell}"
                        );
                        seen_grid.push(cell);
                    }
                    CraftSlot::Inventory(idx) => {
                        assert!(
                            !seen_inventory.contains(&idx),
                            "{layout:?} maps two slots to inventory {idx}"
                        );
                        seen_inventory.push(idx);
                    }
                }
            }

            assert_eq!(seen_grid.len(), layout.grid_cells());
            assert_eq!(
                seen_inventory.len(),
                if layout == CraftLayout::Player {
                    40
                } else {
                    36
                },
                "{layout:?} reached the wrong number of inventory slots"
            );
        }
    }

    fn stack(item_id: i16, count: u8) -> Option<ItemStack> {
        Some(ItemStack {
            item_id,
            count,
            metadata: 0,
            durability: 0,
        })
    }

    #[test]
    fn a_left_click_picks_up_a_whole_stack_and_puts_it_back() {
        let mut slot = stack(5, 32);
        let mut cursor = None;

        swap_or_merge(&mut slot, &mut cursor, 64, true);
        assert!(slot.is_none());
        assert_eq!(cursor.as_ref().unwrap().count, 32);

        swap_or_merge(&mut slot, &mut cursor, 64, true);
        assert_eq!(slot.as_ref().unwrap().count, 32);
        assert!(cursor.is_none());
    }

    #[test]
    fn a_right_click_takes_half_and_places_one() {
        let mut slot = stack(5, 7);
        let mut cursor = None;

        swap_or_merge(&mut slot, &mut cursor, 64, false);
        assert_eq!(cursor.as_ref().unwrap().count, 4, "the larger half");
        assert_eq!(slot.as_ref().unwrap().count, 3);

        swap_or_merge(&mut slot, &mut cursor, 64, false);
        assert_eq!(slot.as_ref().unwrap().count, 4, "one placed");
        assert_eq!(cursor.as_ref().unwrap().count, 3);
    }

    #[test]
    fn merging_stops_at_the_stack_limit() {
        let mut slot = stack(5, 60);
        let mut cursor = stack(5, 10);

        swap_or_merge(&mut slot, &mut cursor, 64, true);
        assert_eq!(slot.as_ref().unwrap().count, 64);
        assert_eq!(cursor.as_ref().unwrap().count, 6, "the rest stays held");
    }

    #[test]
    fn different_items_exchange_rather_than_merge() {
        let mut slot = stack(5, 3);
        let mut cursor = stack(280, 1);

        swap_or_merge(&mut slot, &mut cursor, 64, true);
        assert_eq!(slot.as_ref().unwrap().item_id, 280);
        assert_eq!(cursor.as_ref().unwrap().item_id, 5);
    }

    #[test]
    fn only_the_open_window_can_reach_the_grid() {
        assert_eq!(layout_for(None, 0), Some(CraftLayout::Player));
        assert_eq!(layout_for(None, 3), None);

        let bench = WindowType::Crafting { window_id: 7 };
        assert_eq!(layout_for(Some(&bench), 7), Some(CraftLayout::Bench));

        assert_eq!(
            layout_for(Some(&bench), 0),
            None,
            "window 0 must not address a bench grid"
        );
        assert_eq!(layout_for(Some(&bench), 4), None, "nor any other id");
    }

    #[test]
    fn another_window_being_open_blocks_the_grid_entirely() {
        let chest = WindowType::Chest {
            window_id: 2,
            pos: (0, 64, 0),
        };
        for window_id in [0, 2, 5] {
            assert_eq!(
                layout_for(Some(&chest), window_id),
                None,
                "a chest window must not resolve to a crafting layout"
            );
        }
    }

    #[test]
    fn unstackable_items_exchange() {
        let mut slot = stack(268, 1);
        let mut cursor = stack(268, 1);

        swap_or_merge(&mut slot, &mut cursor, 1, true);
        assert_eq!(slot.as_ref().unwrap().count, 1);
        assert_eq!(
            cursor.as_ref().unwrap().count,
            1,
            "two swords stay two separate swords"
        );
    }
}

#[cfg(test)]
mod apply_tests {
    use super::*;
    use coral_server::crafting::RecipeRegistry;
    use coral_server::items::ItemRegistry;
    use coral_types::ids::items::{COBBLESTONE, PLANKS, STICK};

    struct Harness {
        recipes: RecipeRegistry,
        items: ItemRegistry,
    }

    impl Harness {
        fn new() -> Self {
            Self {
                recipes: RecipeRegistry::vanilla(),
                items: ItemRegistry::new(),
            }
        }

        fn deps(&self) -> CraftDeps<'_> {
            CraftDeps {
                recipes: &self.recipes,
                items: &self.items,
            }
        }
    }

    fn click(slot: i16, button: u8, mode: u8) -> ClickWindow {
        ClickWindow {
            window_id: 0,
            slot,
            button,
            action_number: 1,
            mode,
            clicked_item_id: -1,
        }
    }

    fn stack(item_id: i16, count: u8) -> ItemStack {
        ItemStack {
            item_id,
            count,
            metadata: 0,
            durability: 0,
        }
    }

    struct World {
        grid: CraftingGrid,
        inventory: Inventory,
        cursor: Option<ItemStack>,
    }

    impl World {
        fn player() -> Self {
            Self {
                grid: CraftingGrid::player(),
                inventory: Inventory::new(),
                cursor: None,
            }
        }

        fn bench() -> Self {
            Self {
                grid: CraftingGrid::bench(),
                inventory: Inventory::new(),
                cursor: None,
            }
        }

        fn apply(
            &mut self,
            harness: &Harness,
            layout: CraftLayout,
            click: &ClickWindow,
        ) -> Option<Vec<ItemStack>> {
            apply_crafting_click(
                &mut self.grid,
                &mut self.inventory,
                &mut self.cursor,
                layout,
                click,
                &harness.deps(),
            )
        }

        fn total_of(&self, item_id: i16) -> u32 {
            let in_grid: u32 = self
                .grid
                .cells()
                .iter()
                .flatten()
                .filter(|s| s.item_id == item_id)
                .map(|s| s.count as u32)
                .sum();
            let in_inventory: u32 = self
                .inventory
                .slots
                .iter()
                .flatten()
                .filter(|s| s.item_id == item_id)
                .map(|s| s.count as u32)
                .sum();
            let on_cursor: u32 = self
                .cursor
                .iter()
                .filter(|s| s.item_id == item_id)
                .map(|s| s.count as u32)
                .sum();
            in_grid + in_inventory + on_cursor
        }
    }

    #[test]
    fn placing_planks_and_taking_sticks_works_end_to_end() {
        let harness = Harness::new();
        let mut world = World::player();
        world.cursor = Some(stack(PLANKS, 2));

        assert!(
            world
                .apply(&harness, CraftLayout::Player, &click(1, 1, 0))
                .is_some()
        );
        assert!(
            world
                .apply(&harness, CraftLayout::Player, &click(3, 1, 0))
                .is_some()
        );

        assert_eq!(world.grid.get(0).unwrap().count, 1);
        assert_eq!(world.grid.get(2).unwrap().count, 1);
        assert!(world.cursor.is_none(), "both planks were placed");

        world
            .apply(&harness, CraftLayout::Player, &click(0, 0, 0))
            .expect("the output slot is ours");

        let held = world.cursor.as_ref().expect("sticks on the cursor");
        assert_eq!((held.item_id, held.count), (STICK, 4));
        assert!(world.grid.is_empty(), "the grid was consumed");
    }

    #[test]
    fn the_output_slot_refuses_a_placed_item() {
        let harness = Harness::new();
        let mut world = World::player();
        world.cursor = Some(stack(COBBLESTONE, 1));

        world
            .apply(&harness, CraftLayout::Player, &click(0, 0, 0))
            .expect("handled");

        assert_eq!(
            world.cursor.as_ref().map(|s| s.item_id),
            Some(COBBLESTONE),
            "the cobblestone stays on the cursor"
        );
        assert!(world.grid.is_empty(), "and nothing landed in the grid");
        assert_eq!(world.total_of(COBBLESTONE), 1);
    }

    #[test]
    fn clicking_an_empty_output_changes_nothing() {
        let harness = Harness::new();
        let mut world = World::player();
        world.grid.set(0, Some(stack(COBBLESTONE, 1)));

        world
            .apply(&harness, CraftLayout::Player, &click(0, 0, 0))
            .expect("handled");

        assert!(world.cursor.is_none());
        assert_eq!(world.grid.get(0).unwrap().count, 1, "untouched");
    }

    #[test]
    fn shift_clicking_the_output_crafts_until_the_grid_runs_dry() {
        let harness = Harness::new();
        let mut world = World::player();
        world.grid.set(0, Some(stack(PLANKS, 3)));
        world.grid.set(2, Some(stack(PLANKS, 5)));

        let overflow = world
            .apply(&harness, CraftLayout::Player, &click(0, 0, 1))
            .expect("handled");

        assert!(overflow.is_empty(), "there was room for all of it");
        assert_eq!(world.total_of(STICK), 12, "3 crafts of 4 sticks");
        assert_eq!(
            world.total_of(PLANKS),
            2,
            "3 taken from each stack, so 0 and 2 remain"
        );
        assert!(world.grid.get(0).is_none());
        assert_eq!(world.grid.get(2).unwrap().count, 2);
    }

    #[test]
    fn a_batch_that_cannot_fit_comes_back_to_be_dropped() {
        let harness = Harness::new();
        let mut world = World::player();
        for slot in 0..36 {
            world.inventory.slots[slot] = Some(stack(COBBLESTONE, 64));
        }
        world.grid.set(0, Some(stack(PLANKS, 1)));
        world.grid.set(2, Some(stack(PLANKS, 1)));

        let overflow = world
            .apply(&harness, CraftLayout::Player, &click(0, 0, 1))
            .expect("handled");

        assert_eq!(overflow.len(), 1, "the sticks had nowhere to go");
        assert_eq!(
            (overflow[0].item_id, overflow[0].count),
            (STICK, 4),
            "and they come back intact rather than vanishing"
        );
    }

    #[test]
    fn the_player_window_leaves_the_inventory_alone() {
        let harness = Harness::new();
        let mut world = World::player();
        world.inventory.slots[0] = Some(stack(COBBLESTONE, 10));

        for slot in [5i16, 9, 20, 36, 44] {
            assert!(
                world
                    .apply(&harness, CraftLayout::Player, &click(slot, 0, 0))
                    .is_none(),
                "packet slot {slot} must not be claimed in the player window"
            );
        }
        assert_eq!(
            world.inventory.slots[0].as_ref().unwrap().count,
            10,
            "and nothing was touched"
        );
    }

    #[test]
    fn a_bench_claims_its_inventory_half_and_numbers_it_correctly() {
        let harness = Harness::new();
        let mut world = World::bench();
        world.cursor = Some(stack(COBBLESTONE, 4));

        world
            .apply(&harness, CraftLayout::Bench, &click(10, 0, 0))
            .expect("a bench claims packet slot 10");

        assert_eq!(
            world.inventory.slots[9].as_ref().map(|s| s.count),
            Some(4),
            "packet slot 10 is internal slot 9 at a bench"
        );
        assert!(world.inventory.slots[10].is_none(), "not internal slot 10");
    }

    #[test]
    fn shift_clicking_a_bench_inventory_slot_moves_between_the_halves() {
        let harness = Harness::new();
        let mut world = World::bench();
        world.inventory.slots[9] = Some(stack(COBBLESTONE, 7));

        world
            .apply(&harness, CraftLayout::Bench, &click(10, 0, 1))
            .expect("handled");

        assert!(
            world.inventory.slots[9].is_none(),
            "it left the main inventory"
        );
        assert_eq!(
            world.inventory.slots[0].as_ref().map(|s| s.count),
            Some(7),
            "and arrived in the hotbar"
        );
        assert_eq!(world.total_of(COBBLESTONE), 7, "nothing was duplicated");
    }

    #[test]
    fn shift_clicking_the_hotbar_sends_it_to_the_main_inventory() {
        let harness = Harness::new();
        let mut world = World::bench();
        world.inventory.slots[0] = Some(stack(STICK, 3));

        world
            .apply(&harness, CraftLayout::Bench, &click(37, 0, 1))
            .expect("handled");

        assert!(world.inventory.slots[0].is_none());
        assert_eq!(world.inventory.slots[9].as_ref().map(|s| s.count), Some(3));
        assert_eq!(world.total_of(STICK), 3);
    }

    #[test]
    fn shift_clicking_a_grid_cell_sends_it_to_the_inventory() {
        let harness = Harness::new();
        let mut world = World::player();
        world.grid.set(0, Some(stack(PLANKS, 6)));

        world
            .apply(&harness, CraftLayout::Player, &click(1, 0, 1))
            .expect("handled");

        assert!(world.grid.get(0).is_none(), "the cell was emptied");
        assert_eq!(world.total_of(PLANKS), 6, "and the planks are in the bag");
    }

    #[test]
    fn an_unimplemented_mode_is_inert() {
        let harness = Harness::new();
        let mut world = World::player();
        world.grid.set(0, Some(stack(PLANKS, 1)));
        world.grid.set(2, Some(stack(PLANKS, 1)));

        for mode in [2u8, 3, 4, 5, 6] {
            let before = world.total_of(PLANKS);
            world
                .apply(&harness, CraftLayout::Player, &click(1, 0, mode))
                .expect("claimed, then ignored");
            assert_eq!(
                world.total_of(PLANKS),
                before,
                "mode {mode} must not move anything"
            );
        }
        assert!(world.cursor.is_none());
    }

    #[test]
    fn a_click_outside_the_window_is_not_claimed() {
        let harness = Harness::new();
        let mut world = World::player();
        assert!(
            world
                .apply(&harness, CraftLayout::Player, &click(-999, 0, 0))
                .is_none()
        );
    }

    #[test]
    fn no_click_on_a_grid_cell_changes_how_many_items_exist() {
        let harness = Harness::new();

        for slot in 1i16..=4 {
            for button in [0u8, 1] {
                let mut world = World::player();
                world.grid.set(0, Some(stack(PLANKS, 3)));
                world.cursor = Some(stack(PLANKS, 5));
                let before = world.total_of(PLANKS);

                world
                    .apply(&harness, CraftLayout::Player, &click(slot, button, 0))
                    .expect("handled");

                assert_eq!(
                    world.total_of(PLANKS),
                    before,
                    "slot {slot} button {button} changed the total"
                );
            }
        }
    }
}
