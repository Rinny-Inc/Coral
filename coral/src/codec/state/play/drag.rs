use coral_protocol::packets::play::inventory::ItemStack;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    /// Spread the held stack evenly
    Left,
    /// One item per slot
    Right,
    /// Fill every slot (Creative Only)
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragStep {
    Start(DragKind),
    Add(DragKind),
    End(DragKind),
}

/// Decode a mode-5 button
pub fn classify(button: u8) -> Option<DragStep> {
    let kind = match button / 4 {
        0 => DragKind::Left,
        1 => DragKind::Right,
        2 => DragKind::Middle,
        _ => return None,
    };
    match button & 3 {
        0 => Some(DragStep::Start(kind)),
        1 => Some(DragStep::Add(kind)),
        2 => Some(DragStep::End(kind)),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct Drag {
    pub kind: DragKind,
    /// Packet slots swept in order without repeat
    pub painted: Vec<i16>,
}
impl Drag {
    pub fn new(kind: DragKind) -> Self {
        Self {
            kind,
            painted: Vec::new(),
        }
    }

    /// Record a swept slot
    pub fn paint(&mut self, slot: i16) {
        if self.painted.contains(&slot) {
            return;
        }
        self.painted.push(slot);
    }
}

#[derive(Debug, Clone)]
pub struct DragPlan {
    /// `(packet slot, the count that slot ends up holding)`
    pub placements: Vec<(i16, u8)>,
    /// What stays on the cursor afterwards
    pub cursor_left: Option<ItemStack>,
}
impl DragPlan {
    fn untouched(cursor: &ItemStack) -> Self {
        Self {
            placements: Vec::new(),
            cursor_left: Some(cursor.clone()),
        }
    }
}

fn same_item(a: &ItemStack, b: &ItemStack) -> bool {
    a.item_id == b.item_id && a.metadata == b.metadata && a.durability == b.durability
}

/// Work out where a finished drag puts things
pub fn plan_drag(
    kind: DragKind,
    painted: &[(i16, Option<ItemStack>)],
    cursor: &ItemStack,
    max_stack: u8,
) -> DragPlan {
    if kind == DragKind::Middle {
        return DragPlan::untouched(cursor);
    }

    let eligible: Vec<(i16, u8)> = painted
        .iter()
        .filter_map(|(slot, held)| match held {
            None => Some((*slot, 0)),
            Some(existing) if same_item(existing, cursor) && existing.count < max_stack => {
                Some((*slot, existing.count))
            }
            Some(_) => None,
        })
        .collect();

    if eligible.is_empty() || cursor.count == 0 {
        return DragPlan::untouched(cursor);
    }

    let share = match kind {
        DragKind::Left => cursor.count / eligible.len() as u8,
        DragKind::Right => 1,
        DragKind::Middle => unreachable!("refused above"),
    };

    if share == 0 {
        return DragPlan::untouched(cursor);
    }

    let mut placements = Vec::with_capacity(eligible.len());
    let mut remaining = cursor.count;

    for (slot, already) in eligible {
        if remaining == 0 {
            break;
        }
        let room = max_stack - already;
        let giving = share.min(room).min(remaining);
        if giving == 0 {
            continue;
        }
        placements.push((slot, already + giving));
        remaining -= giving;
    }

    DragPlan {
        placements,
        cursor_left: (remaining > 0).then(|| ItemStack {
            count: remaining,
            ..cursor.clone()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(item_id: i16, count: u8) -> ItemStack {
        ItemStack {
            item_id,
            count,
            metadata: 0,
            durability: 0,
        }
    }

    fn empty(slots: &[i16]) -> Vec<(i16, Option<ItemStack>)> {
        slots.iter().map(|s| (*s, None)).collect()
    }

    fn accounted(plan: &DragPlan, painted: &[(i16, Option<ItemStack>)]) -> u32 {
        let placed: u32 = plan
            .placements
            .iter()
            .map(|(slot, count)| {
                let before = painted
                    .iter()
                    .find(|(s, _)| s == slot)
                    .and_then(|(_, held)| held.as_ref())
                    .map(|s| s.count as u32)
                    .unwrap_or(0);
                *count as u32 - before
            })
            .sum();
        let left = plan.cursor_left.as_ref().map_or(0, |s| s.count as u32);
        placed + left
    }

    #[test]
    fn the_button_encodes_both_the_step_and_the_mouse_button() {
        use DragKind::*;
        use DragStep::*;

        assert_eq!(classify(0), Some(Start(Left)));
        assert_eq!(classify(1), Some(Add(Left)));
        assert_eq!(classify(2), Some(End(Left)));
        assert_eq!(classify(4), Some(Start(Right)));
        assert_eq!(classify(5), Some(Add(Right)));
        assert_eq!(classify(6), Some(End(Right)));
        assert_eq!(classify(8), Some(Start(Middle)));
        assert_eq!(classify(10), Some(End(Middle)));

        for button in [3u8, 7, 11, 12, 255] {
            assert_eq!(classify(button), None, "button {button}");
        }
    }

    #[test]
    fn painting_the_same_slot_twice_only_counts_once() {
        let mut drag = Drag::new(DragKind::Left);
        drag.paint(9);
        drag.paint(10);
        drag.paint(9);
        assert_eq!(drag.painted, vec![9, 10], "a slot gets one share, not two");
    }

    #[test]
    fn a_left_drag_splits_evenly() {
        let painted = empty(&[9, 10, 11]);
        let plan = plan_drag(DragKind::Left, &painted, &stack(5, 9), 64);

        assert_eq!(plan.placements, vec![(9, 3), (10, 3), (11, 3)]);
        assert!(plan.cursor_left.is_none(), "all nine were spread");
        assert_eq!(accounted(&plan, &painted), 9);
    }

    #[test]
    fn a_left_drag_keeps_the_remainder_on_the_cursor() {
        let painted = empty(&[9, 10, 11]);
        let plan = plan_drag(DragKind::Left, &painted, &stack(5, 10), 64);

        assert_eq!(plan.placements, vec![(9, 3), (10, 3), (11, 3)]);
        assert_eq!(plan.cursor_left.as_ref().unwrap().count, 1);
        assert_eq!(accounted(&plan, &painted), 10, "ten in, ten out");
    }

    #[test]
    fn a_right_drag_gives_one_each() {
        let painted = empty(&[9, 10, 11]);
        let plan = plan_drag(DragKind::Right, &painted, &stack(5, 10), 64);

        assert_eq!(plan.placements, vec![(9, 1), (10, 1), (11, 1)]);
        assert_eq!(plan.cursor_left.as_ref().unwrap().count, 7);
        assert_eq!(accounted(&plan, &painted), 10);
    }

    #[test]
    fn a_right_drag_stops_when_the_stack_runs_out() {
        let painted = empty(&[9, 10, 11, 12, 13]);
        let plan = plan_drag(DragKind::Right, &painted, &stack(5, 2), 64);

        assert_eq!(plan.placements, vec![(9, 1), (10, 1)]);
        assert!(plan.cursor_left.is_none());
        assert_eq!(accounted(&plan, &painted), 2);
    }

    #[test]
    fn a_drag_adds_to_matching_stacks() {
        let painted = vec![
            (9, Some(stack(5, 10))),
            (10, Some(stack(5, 60))),
            (11, None),
        ];
        let plan = plan_drag(DragKind::Left, &painted, &stack(5, 9), 64);

        assert_eq!(plan.placements, vec![(9, 13), (10, 63), (11, 3)]);
        assert_eq!(accounted(&plan, &painted), 9, "nothing invented");
    }

    #[test]
    fn slots_holding_something_else_are_skipped() {
        let painted = vec![
            (9, Some(stack(999, 1))),
            (10, None),
            (11, Some(stack(5, 1))),
        ];
        let plan = plan_drag(DragKind::Left, &painted, &stack(5, 4), 64);

        let touched: Vec<i16> = plan.placements.iter().map(|(s, _)| *s).collect();
        assert_eq!(touched, vec![10, 11], "slot 9 holds a different item");
        assert_eq!(accounted(&plan, &painted), 4);
    }

    #[test]
    fn a_full_matching_slot_is_skipped() {
        let painted = vec![(9, Some(stack(5, 64))), (10, None)];
        let plan = plan_drag(DragKind::Left, &painted, &stack(5, 4), 64);

        assert_eq!(plan.placements, vec![(10, 4)]);
        assert_eq!(accounted(&plan, &painted), 4);
    }

    #[test]
    fn a_left_drag_over_too_many_slots_does_nothing() {
        let painted = empty(&[9, 10, 11, 12]);
        let plan = plan_drag(DragKind::Left, &painted, &stack(5, 3), 64);

        assert!(plan.placements.is_empty());
        assert_eq!(plan.cursor_left.as_ref().unwrap().count, 3);
    }

    #[test]
    fn a_drag_over_nothing_does_nothing() {
        let plan = plan_drag(DragKind::Left, &[], &stack(5, 10), 64);
        assert!(plan.placements.is_empty());
        assert_eq!(plan.cursor_left.as_ref().unwrap().count, 10);
    }

    #[test]
    fn a_middle_drag_is_refused() {
        let painted = empty(&[9, 10]);
        let plan = plan_drag(DragKind::Middle, &painted, &stack(5, 1), 64);

        assert!(
            plan.placements.is_empty(),
            "creative fill is not allowed yet"
        );
        assert_eq!(plan.cursor_left.as_ref().unwrap().count, 1);
    }

    #[test]
    fn unstackable_items_go_one_per_slot() {
        let painted = empty(&[9, 10, 11]);
        let plan = plan_drag(DragKind::Left, &painted, &stack(268, 3), 1);

        assert_eq!(plan.placements, vec![(9, 1), (10, 1), (11, 1)]);
        assert!(plan.cursor_left.is_none());
        assert_eq!(accounted(&plan, &painted), 3);
    }

    #[test]
    fn every_drag_conserves_the_stack() {
        for kind in [DragKind::Left, DragKind::Right] {
            for count in [1u8, 2, 3, 7, 10, 31, 64] {
                for slot_count in 1usize..=6 {
                    for existing in [0u8, 1, 50, 63, 64] {
                        let painted: Vec<(i16, Option<ItemStack>)> = (0..slot_count)
                            .map(|i| {
                                let slot = 9 + i as i16;
                                if existing == 0 {
                                    (slot, None)
                                } else {
                                    (slot, Some(stack(5, existing)))
                                }
                            })
                            .collect();

                        let cursor = stack(5, count);
                        let plan = plan_drag(kind, &painted, &cursor, 64);

                        assert_eq!(
                            accounted(&plan, &painted),
                            count as u32,
                            "{kind:?} count={count} slots={slot_count} existing={existing}"
                        );

                        for (_, result) in &plan.placements {
                            assert!(*result <= 64, "{kind:?} overfilled a slot to {result}");
                        }
                    }
                }
            }
        }
    }
}
