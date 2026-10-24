use coral_protocol::packets::play::inventory::{ClickWindow, Inventory, ItemStack};

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
) -> ClickOutcome {
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
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                slots: [const { None }; 44],
                cursor: None,
                dropped: Vec::new(),
            }
        }
        fn go(&mut self, c: ClickWindow) -> ClickOutcome {
            apply_click(&mut self.slots, &mut self.cursor, &c, &mut self.dropped)
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
        for mode in [1u8, 2, 3, 5, 6] {
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
}
