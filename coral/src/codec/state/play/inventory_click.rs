use coral_protocol::packets::play::inventory::{ClickWindow, Inventory, ItemStack};

use crate::codec::state::play::drag::{Drag, DragStep, classify, plan_drag};

#[derive(Debug, PartialEq, Eq)]
pub enum ClickOutcome {
    Applied,
    AppliedArmorChanged,
    Resync,
}

const OUTSIDE: i16 = -999;

fn split_half(stack: &mut ItemStack) -> ItemStack {
    let taken = stack.count.div_ceil(2);
    stack.count -= taken;
    ItemStack {
        count: taken,
        ..*stack
    }
}

fn same_item(a: &ItemStack, b: &ItemStack) -> bool {
    a.item_id == b.item_id && a.metadata == b.metadata && a.durability == b.durability
}

pub fn apply_click(
    slots: &mut [Option<ItemStack>; 44],
    cursor: &mut Option<ItemStack>,
    click: &ClickWindow,
    dropped: &mut Vec<ItemStack>,
    drag: &mut Option<Drag>,
) -> ClickOutcome {
    if click.mode != 5 && drag.is_some() {
        *drag = None;
    }

    if click.mode == 5 {
        return apply_drag_step(slots, cursor, click, drag);
    }
    if click.slot == OUTSIDE {
        return match (click.mode, cursor.take()) {
            (0, Some(mut held)) => {
                if click.button == 1 {
                    let one = ItemStack { count: 1, ..held };
                    held.count -= 1;
                    if held.count > 0 {
                        *cursor = Some(held);
                    }
                    dropped.push(one);
                } else {
                    dropped.push(held);
                }
                ClickOutcome::Applied
            }
            (_, held) => {
                *cursor = held;
                ClickOutcome::Resync
            }
        };
    }

    let Some(idx) = Inventory::packet_to_internal(click.slot) else {
        return ClickOutcome::Resync;
    };
    let armor = Inventory::ARMOR_SLOTS.contains(&idx);

    let outcome = |changed_armor: bool| {
        if changed_armor {
            ClickOutcome::AppliedArmorChanged
        } else {
            ClickOutcome::Applied
        }
    };

    match (click.mode, click.button) {
        (0, 0) => {
            match (cursor.take(), slots[idx].take()) {
                (None, slot) => *cursor = slot,
                (held, None) => slots[idx] = held,
                (Some(held), Some(mut in_slot)) => {
                    if same_item(&held, &in_slot) && in_slot.count < 64 {
                        let room = 64 - in_slot.count;
                        let moved = room.min(held.count);
                        in_slot.count += moved;
                        slots[idx] = Some(in_slot);
                        let left = held.count - moved;
                        if left > 0 {
                            *cursor = Some(ItemStack {
                                count: left,
                                ..held
                            });
                        }
                    } else {
                        slots[idx] = Some(held);
                        *cursor = Some(in_slot);
                    }
                }
            }
            outcome(armor)
        }
        (0, 1) => {
            match (cursor.take(), slots[idx].take()) {
                (None, Some(mut in_slot)) => {
                    let half = split_half(&mut in_slot);
                    *cursor = Some(half);
                    if in_slot.count > 0 {
                        slots[idx] = Some(in_slot);
                    }
                }
                (Some(mut held), None) => {
                    slots[idx] = Some(ItemStack { count: 1, ..held });
                    held.count -= 1;
                    if held.count > 0 {
                        *cursor = Some(held);
                    }
                }
                (Some(mut held), Some(mut in_slot)) => {
                    if same_item(&held, &in_slot) && in_slot.count < 64 {
                        in_slot.count += 1;
                        held.count -= 1;
                        slots[idx] = Some(in_slot);
                        if held.count > 0 {
                            *cursor = Some(held);
                        }
                    } else {
                        slots[idx] = Some(held);
                        *cursor = Some(in_slot);
                    }
                }
                (None, None) => {}
            }
            outcome(armor)
        }
        (2, button) => {
            let hotbar = button as usize;
            if hotbar >= 9 {
                return ClickOutcome::Resync;
            }
            if hotbar == idx {
                return ClickOutcome::Applied;
            }
            slots.swap(idx, hotbar);
            outcome(armor || Inventory::ARMOR_SLOTS.contains(&hotbar))
        }
        (4, button) => {
            let Some(mut in_slot) = slots[idx].take() else {
                return ClickOutcome::Applied;
            };
            if button == 0 && in_slot.count > 1 {
                dropped.push(ItemStack {
                    count: 1,
                    ..in_slot
                });
                in_slot.count -= 1;
                slots[idx] = Some(in_slot);
            } else {
                dropped.push(in_slot);
            }
            outcome(armor)
        }
        _ => ClickOutcome::Resync,
    }
}

fn apply_drag_step(
    slots: &mut [Option<ItemStack>; 44],
    cursor: &mut Option<ItemStack>,
    click: &ClickWindow,
    drag: &mut Option<Drag>,
) -> ClickOutcome {
    let Some(step) = classify(click.button) else {
        *drag = None;
        return ClickOutcome::Resync;
    };

    match step {
        DragStep::Start(kind) => {
            if cursor.is_none() {
                *drag = None;
                return ClickOutcome::Resync;
            }
            *drag = Some(Drag::new(kind));
            ClickOutcome::Applied
        }
        DragStep::Add(kind) => {
            let Some(active) = drag.as_mut() else {
                return ClickOutcome::Resync;
            };
            if active.kind != kind {
                *drag = None;
                return ClickOutcome::Resync;
            }
            if Inventory::packet_to_internal(click.slot).is_some() {
                active.paint(click.slot);
            }
            ClickOutcome::Applied
        }
        DragStep::End(kind) => {
            let Some(active) = drag.take() else {
                return ClickOutcome::Resync;
            };
            if active.kind != kind {
                return ClickOutcome::Resync;
            }
            let Some(held) = cursor.clone() else {
                return ClickOutcome::Resync;
            };

            let painted: Vec<(i16, Option<ItemStack>)> = active
                .painted
                .iter()
                .filter_map(|packet_slot| {
                    let idx = Inventory::packet_to_internal(*packet_slot)?;
                    Some((*packet_slot, slots[idx].clone()))
                })
                .collect();

            let plan = plan_drag(active.kind, &painted, &held, 64);

            let mut changed_armor = false;
            for (packet_slot, count) in plan.placements {
                let Some(idx) = Inventory::packet_to_internal(packet_slot) else {
                    continue;
                };
                changed_armor |= Inventory::ARMOR_SLOTS.contains(&idx);
                slots[idx] = Some(ItemStack {
                    count,
                    ..held.clone()
                });
            }
            *cursor = plan.cursor_left;

            if changed_armor {
                ClickOutcome::AppliedArmorChanged
            } else {
                ClickOutcome::Applied
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOTBAR_0: i16 = 36; // internal 0
    const MAIN_0: i16 = 9; // internal 9
    const HELMET: i16 = 5; // internal 36

    fn stack(id: i16, count: u8) -> ItemStack {
        ItemStack {
            item_id: id,
            count,
            metadata: 0,
            durability: 0,
        }
    }

    fn click(slot: i16, button: u8, mode: u8) -> ClickWindow {
        ClickWindow {
            window_id: 0,
            slot,
            button,
            action_number: 1,
            mode,
            clicked_item_id: 0,
        }
    }

    struct Fixture {
        slots: [Option<ItemStack>; 44],
        cursor: Option<ItemStack>,
        dropped: Vec<ItemStack>,
        drag: Option<Drag>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                slots: [const { None }; 44],
                cursor: None,
                dropped: Vec::new(),
                drag: None,
            }
        }
        fn go(&mut self, c: ClickWindow) -> ClickOutcome {
            apply_click(
                &mut self.slots,
                &mut self.cursor,
                &c,
                &mut self.dropped,
                &mut self.drag,
            )
        }
        fn total_of(&self, item_id: i16) -> u32 {
            let in_slots: u32 = self
                .slots
                .iter()
                .flatten()
                .filter(|s| s.item_id == item_id)
                .map(|s| s.count as u32)
                .sum();
            let held: u32 = self
                .cursor
                .iter()
                .filter(|s| s.item_id == item_id)
                .map(|s| s.count as u32)
                .sum();
            in_slots + held
        }
        fn at(&self, internal: usize) -> Option<(i16, u8)> {
            self.slots[internal].as_ref().map(|s| (s.item_id, s.count))
        }
        fn held(&self) -> Option<(i16, u8)> {
            self.cursor.as_ref().map(|s| (s.item_id, s.count))
        }
    }

    #[test]
    fn left_click_picks_up_and_puts_down() {
        let mut f = Fixture::new();
        f.slots[0] = Some(stack(1, 32));

        assert_eq!(f.go(click(HOTBAR_0, 0, 0)), ClickOutcome::Applied);
        assert_eq!(f.held(), Some((1, 32)));
        assert_eq!(f.at(0), None);

        assert_eq!(f.go(click(MAIN_0, 0, 0)), ClickOutcome::Applied);
        assert_eq!(f.at(9), Some((1, 32)));
        assert_eq!(f.held(), None);
    }

    #[test]
    fn left_click_on_a_different_item_swaps() {
        let mut f = Fixture::new();
        f.slots[0] = Some(stack(1, 10));
        f.cursor = Some(stack(2, 5));

        f.go(click(HOTBAR_0, 0, 0));
        assert_eq!(f.at(0), Some((2, 5)));
        assert_eq!(f.held(), Some((1, 10)));
    }

    #[test]
    fn left_click_on_the_same_item_merges_up_to_64() {
        let mut f = Fixture::new();
        f.slots[0] = Some(stack(1, 60));
        f.cursor = Some(stack(1, 10));

        f.go(click(HOTBAR_0, 0, 0));
        assert_eq!(f.at(0), Some((1, 64)));
        assert_eq!(f.held(), Some((1, 6)), "the remainder stays on the cursor");
    }

    #[test]
    fn right_click_takes_half_rounding_up() {
        let mut f = Fixture::new();
        f.slots[0] = Some(stack(1, 7));

        f.go(click(HOTBAR_0, 1, 0));
        assert_eq!(f.held(), Some((1, 4)));
        assert_eq!(f.at(0), Some((1, 3)));
    }

    #[test]
    fn right_click_taking_a_whole_single_item_empties_the_slot() {
        let mut f = Fixture::new();
        f.slots[0] = Some(stack(1, 1));
        f.go(click(HOTBAR_0, 1, 0));
        assert_eq!(f.held(), Some((1, 1)));
        assert_eq!(f.at(0), None);
    }

    #[test]
    fn right_click_places_one_at_a_time() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 3));

        f.go(click(HOTBAR_0, 1, 0));
        assert_eq!(f.at(0), Some((1, 1)));
        assert_eq!(f.held(), Some((1, 2)));

        f.go(click(HOTBAR_0, 1, 0));
        assert_eq!(f.at(0), Some((1, 2)));
        assert_eq!(f.held(), Some((1, 1)));

        f.go(click(HOTBAR_0, 1, 0));
        assert_eq!(f.at(0), Some((1, 3)));
        assert_eq!(f.held(), None, "the last one leaves an empty cursor");
    }

    #[test]
    fn item_count_is_conserved() {
        let mut f = Fixture::new();
        f.slots[0] = Some(stack(1, 40));
        f.slots[9] = Some(stack(1, 40));
        let total = 80u32;

        for c in [
            click(HOTBAR_0, 0, 0),
            click(MAIN_0, 0, 0),
            click(HOTBAR_0, 1, 0),
            click(MAIN_0, 1, 0),
            click(HOTBAR_0, 0, 0),
        ] {
            f.go(c);
            let in_slots: u32 = f.slots.iter().flatten().map(|s| s.count as u32).sum();
            let on_cursor = f.cursor.as_ref().map_or(0, |s| s.count as u32);
            let on_floor: u32 = f.dropped.iter().map(|s| s.count as u32).sum();
            assert_eq!(
                in_slots + on_cursor + on_floor,
                total,
                "items appeared or vanished"
            );
        }
    }

    #[test]
    fn touching_armor_reports_it() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(310, 1));
        assert_eq!(f.go(click(HELMET, 0, 0)), ClickOutcome::AppliedArmorChanged);
        assert_eq!(f.at(Inventory::ARMOR_HELMET), Some((310, 1)));
    }

    #[test]
    fn q_drops_one_and_ctrl_q_drops_the_stack() {
        let mut f = Fixture::new();
        f.slots[0] = Some(stack(1, 5));

        f.go(click(HOTBAR_0, 0, 4));
        assert_eq!(f.at(0), Some((1, 4)));
        assert_eq!(f.dropped.len(), 1);
        assert_eq!(f.dropped[0].count, 1);

        f.go(click(HOTBAR_0, 1, 4));
        assert_eq!(f.at(0), None);
        assert_eq!(f.dropped[1].count, 4);
    }

    #[test]
    fn clicking_outside_throws_the_cursor_away() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 9));
        assert_eq!(f.go(click(OUTSIDE, 0, 0)), ClickOutcome::Applied);
        assert_eq!(f.held(), None);
        assert_eq!(f.dropped[0].count, 9);
    }

    #[test]
    fn right_click_outside_throws_one() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 9));
        f.go(click(OUTSIDE, 1, 0));
        assert_eq!(f.held(), Some((1, 8)));
        assert_eq!(f.dropped[0].count, 1);
    }

    #[test]
    fn unmodelled_modes_ask_for_a_resync_and_change_nothing() {
        for mode in [1u8, 3, 6] {
            let mut f = Fixture::new();
            f.slots[0] = Some(stack(1, 10));
            assert_eq!(
                f.go(click(HOTBAR_0, 0, mode)),
                ClickOutcome::Resync,
                "mode {mode}"
            );
            assert_eq!(f.at(0), Some((1, 10)), "mode {mode} mutated the inventory");
            assert!(f.dropped.is_empty());
        }
    }

    #[test]
    fn an_unmapped_slot_is_a_resync_not_a_panic() {
        let mut f = Fixture::new();
        assert_eq!(f.go(click(-5, 0, 0)), ClickOutcome::Resync);
        assert_eq!(f.go(click(999, 0, 0)), ClickOutcome::Resync);
    }

    fn packet_for(internal: usize) -> i16 {
        Inventory::internal_to_packet(internal)
    }

    #[test]
    fn a_number_key_swaps_the_slot_with_that_hotbar_slot() {
        let mut f = Fixture::new();
        f.slots[20] = Some(stack(1, 10)); // a main-inventory slot
        f.slots[3] = Some(stack(2, 5)); // hotbar slot 3

        assert_eq!(f.go(click(packet_for(20), 3, 2)), ClickOutcome::Applied);

        assert_eq!(f.at(20), Some((2, 5)), "the hotbar item came up");
        assert_eq!(f.at(3), Some((1, 10)), "and the other went down");
        assert_eq!(f.total_of(1) + f.total_of(2), 15, "nothing created");
    }

    #[test]
    fn a_number_key_onto_an_empty_hotbar_slot_moves_it() {
        let mut f = Fixture::new();
        f.slots[20] = Some(stack(1, 7));

        f.go(click(packet_for(20), 0, 2));

        assert_eq!(f.at(20), None);
        assert_eq!(f.at(0), Some((1, 7)));
    }

    #[test]
    fn a_number_key_outside_the_hotbar_resyncs() {
        let mut f = Fixture::new();
        f.slots[20] = Some(stack(1, 7));

        for button in [9u8, 10, 255] {
            assert_eq!(
                f.go(click(packet_for(20), button, 2)),
                ClickOutcome::Resync,
                "button {button}"
            );
            assert_eq!(f.at(20), Some((1, 7)), "and nothing moved");
        }
    }

    #[test]
    fn a_left_drag_spreads_the_held_stack_across_the_painted_slots() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 9));

        assert_eq!(f.go(click(-999, 0, 5)), ClickOutcome::Applied, "start");
        f.go(click(packet_for(9), 1, 5));
        f.go(click(packet_for(10), 1, 5));
        f.go(click(packet_for(11), 1, 5));
        assert_eq!(f.go(click(-999, 2, 5)), ClickOutcome::Applied, "end");

        assert_eq!(f.at(9), Some((1, 3)));
        assert_eq!(f.at(10), Some((1, 3)));
        assert_eq!(f.at(11), Some((1, 3)));
        assert!(f.held().is_none());
        assert_eq!(f.total_of(1), 9, "nothing created or lost");
    }

    #[test]
    fn a_right_drag_places_one_per_slot_and_keeps_the_rest() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 10));

        f.go(click(-999, 4, 5));
        f.go(click(packet_for(9), 5, 5));
        f.go(click(packet_for(10), 5, 5));
        f.go(click(-999, 6, 5));

        assert_eq!(f.at(9), Some((1, 1)));
        assert_eq!(f.at(10), Some((1, 1)));
        assert_eq!(f.held(), Some((1, 8)));
        assert_eq!(f.total_of(1), 10);
    }

    #[test]
    fn an_abandoned_drag_changes_nothing() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 9));

        f.go(click(-999, 0, 5));
        f.go(click(packet_for(9), 1, 5));
        f.go(click(packet_for(10), 1, 5));

        f.go(click(packet_for(20), 0, 0));

        assert_eq!(f.at(9), None, "nothing was placed");
        assert_eq!(f.at(10), None);
        assert!(f.drag.is_none(), "and the gesture was discarded");
        assert_eq!(f.total_of(1), 9);
    }

    #[test]
    fn an_end_step_without_a_start_does_nothing() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 9));
        f.slots[9] = None;

        assert_eq!(f.go(click(-999, 2, 5)), ClickOutcome::Resync);
        assert_eq!(f.held(), Some((1, 9)), "the stack is untouched");
        assert_eq!(f.at(9), None);
    }

    #[test]
    fn a_drag_started_with_an_empty_cursor_is_refused() {
        let mut f = Fixture::new();
        assert_eq!(f.go(click(-999, 0, 5)), ClickOutcome::Resync);
        assert!(f.drag.is_none());
    }

    #[test]
    fn changing_button_mid_drag_abandons_it() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 9));

        f.go(click(-999, 0, 5)); // start left
        f.go(click(packet_for(9), 1, 5)); // add left
        assert_eq!(
            f.go(click(packet_for(10), 5, 5)),
            ClickOutcome::Resync,
            "an add with the right button"
        );
        assert!(f.drag.is_none());
        assert_eq!(f.total_of(1), 9);
        assert_eq!(f.at(9), None, "and nothing was placed");
    }

    #[test]
    fn an_unassigned_drag_button_resyncs() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 4));
        for button in [3u8, 7, 11, 200] {
            assert_eq!(
                f.go(click(-999, button, 5)),
                ClickOutcome::Resync,
                "button {button}"
            );
        }
        assert_eq!(f.held(), Some((1, 4)));
    }

    #[test]
    fn a_drag_skips_slots_holding_another_item() {
        let mut f = Fixture::new();
        f.cursor = Some(stack(1, 4));
        f.slots[9] = Some(stack(999, 1));

        f.go(click(-999, 0, 5));
        f.go(click(packet_for(9), 1, 5));
        f.go(click(packet_for(10), 1, 5));
        f.go(click(-999, 2, 5));

        assert_eq!(f.at(9), Some((999, 1)), "left alone");
        assert_eq!(f.at(10), Some((1, 4)), "and all four went to the other");
        assert_eq!(f.total_of(1), 4);
    }
}
