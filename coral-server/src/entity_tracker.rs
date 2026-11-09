use std::collections::HashMap;

use coral_types::dist_sq_xz;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum EntityType {
    Player,
    Mob,
    Item,
    ExperienceOrb,
}

pub struct TrackedEntity {
    pub entity_id: i32,
    pub uuid: Option<Uuid>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub entity_type: EntityType,
    pub tracking_range: f64,
}

impl TrackedEntity {
    pub fn player(entity_id: i32, uuid: Uuid, x: f64, y: f64, z: f64, range: f64) -> Self {
        Self {
            entity_id,
            uuid: Some(uuid),
            x,
            y,
            z,
            entity_type: EntityType::Player,
            tracking_range: range,
        }
    }

    pub fn item(entity_id: i32, x: f64, y: f64, z: f64, range: f64) -> Self {
        Self {
            entity_id,
            uuid: None,
            x,
            y,
            z,
            entity_type: EntityType::Item,
            tracking_range: range,
        }
    }

    pub fn mob(entity_id: i32, x: f64, y: f64, z: f64, range: f64) -> Self {
        Self {
            entity_id,
            uuid: None,
            x,
            y,
            z,
            entity_type: EntityType::Mob,
            tracking_range: range,
        }
    }

    pub fn experience_orb(entity_id: i32, x: f64, y: f64, z: f64, range: f64) -> Self {
        Self {
            entity_id,
            uuid: None,
            x,
            y,
            z,
            entity_type: EntityType::ExperienceOrb,
            tracking_range: range,
        }
    }
}

#[inline]
pub fn within_tracking_range(x: f64, z: f64, viewer_x: f64, viewer_z: f64, range: f64) -> bool {
    dist_sq_xz(x, z, viewer_x, viewer_z) <= range * range
}

pub struct EntityTracker {
    pub entities: HashMap<i32, TrackedEntity>,
}
impl EntityTracker {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }

    pub fn track(&mut self, entity: TrackedEntity) {
        self.entities.insert(entity.entity_id, entity);
    }

    pub fn untrack(&mut self, entity_id: i32) {
        self.entities.remove(&entity_id);
    }

    pub fn untrack_many(&mut self, entity_ids: &[i32]) {
        for id in entity_ids {
            self.entities.remove(id);
        }
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn is_tracked(&self, entity_id: i32) -> bool {
        self.entities.contains_key(&entity_id)
    }

    pub fn update_position(&mut self, entity_id: i32, x: f64, y: f64, z: f64) {
        if let Some(e) = self.entities.get_mut(&entity_id) {
            e.x = x;
            e.y = y;
            e.z = z;
        }
    }

    pub fn is_visible_to(&self, entity_id: i32, viewer_x: f64, viewer_z: f64) -> bool {
        if let Some(e) = self.entities.get(&entity_id) {
            within_tracking_range(e.x, e.z, viewer_x, viewer_z, e.tracking_range)
        } else {
            false
        }
    }

    pub fn get_visible_for(&self, viewer_x: f64, viewer_z: f64) -> Vec<&TrackedEntity> {
        self.entities
            .values()
            .filter(|e| within_tracking_range(e.x, e.z, viewer_x, viewer_z, e.tracking_range))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::drops::DroppedItems;

    fn xorshift(state: &mut u64) -> u64 {
        *state ^= *state >> 12;
        *state ^= *state << 25;
        *state ^= *state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    #[test]
    fn untrack_drops_the_entity_from_visibility() {
        let mut tracker = EntityTracker::new();
        tracker.track(TrackedEntity::item(7, 0.0, 64.0, 0.0, 64.0));

        assert!(tracker.is_visible_to(7, 1.0, 1.0), "in range while tracked");

        tracker.untrack(7);

        assert!(!tracker.is_tracked(7));
        assert!(
            !tracker.is_visible_to(7, 1.0, 1.0),
            "an untracked entity is invisible, which is why spawn must track \
             before it broadcasts"
        );
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn untrack_many_takes_only_the_ids_it_was_given() {
        let mut tracker = EntityTracker::new();
        for eid in 0..10 {
            tracker.track(TrackedEntity::item(eid, 0.0, 64.0, 0.0, 64.0));
        }

        tracker.untrack_many(&[1, 3, 5, 7, 9]);

        assert_eq!(tracker.len(), 5);
        for eid in [1, 3, 5, 7, 9] {
            assert!(!tracker.is_tracked(eid), "{eid} should be gone");
        }
        for eid in [0, 2, 4, 6, 8] {
            assert!(tracker.is_tracked(eid), "{eid} should have survived");
        }
    }

    #[test]
    fn untracking_an_unknown_id_is_a_no_op() {
        let mut tracker = EntityTracker::new();
        tracker.track(TrackedEntity::item(1, 0.0, 64.0, 0.0, 64.0));

        tracker.untrack(1);
        tracker.untrack(1);
        tracker.untrack_many(&[1, 99, -4]);

        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn the_tracker_never_outgrows_the_live_item_set() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut tracker = EntityTracker::new();
        let mut items = DroppedItems::new();
        let mut live: Vec<i32> = vec![];
        let mut next_eid = 1i32;

        for step in 0..4000 {
            let roll = xorshift(&mut seed) % 100;

            if roll < 55 || live.is_empty() {
                let eid = next_eid;
                next_eid += 1;
                let x = (xorshift(&mut seed) % 2000) as f64 - 1000.0;
                let z = (xorshift(&mut seed) % 2000) as f64 - 1000.0;

                tracker.track(TrackedEntity::item(eid, x, 64.0, z, 64.0));
                items.insert((eid, x, 64.0, z, 1, 1, 0));
                live.push(eid);
            } else if roll < 85 {
                let idx = (xorshift(&mut seed) as usize) % live.len();
                let eid = live.swap_remove(idx);
                items.remove(eid);
                tracker.untrack_many(&[eid]);
            } else {
                let take = ((xorshift(&mut seed) as usize) % live.len()) + 1;
                let batch: Vec<i32> = live.drain(..take).collect();
                for eid in &batch {
                    items.remove(*eid);
                }
                tracker.untrack_many(&batch);
            }

            assert_eq!(
                tracker.len(),
                live.len(),
                "tracker drifted from the live set at step {step}"
            );
            assert_eq!(
                items.len(),
                live.len(),
                "item index drifted from the live set at step {step}"
            );
        }

        tracker.untrack_many(&live);
        assert!(
            tracker.is_empty(),
            "tracker should be empty, has {}",
            tracker.len()
        );
    }

    #[test]
    fn an_orb_carries_its_own_tracking_range() {
        let mut tracker = EntityTracker::new();
        tracker.track(TrackedEntity::experience_orb(1, 0.0, 64.0, 0.0, 16.0));
        tracker.track(TrackedEntity::item(2, 0.0, 64.0, 0.0, 64.0));

        assert_eq!(tracker.entities[&1].entity_type, EntityType::ExperienceOrb);

        assert!(!tracker.is_visible_to(1, 32.0, 0.0), "orb is out of range");
        assert!(
            tracker.is_visible_to(2, 32.0, 0.0),
            "item is still in range"
        );
    }

    #[test]
    fn orb_visibility_is_unaffected_by_vertical_movement() {
        let mut tracker = EntityTracker::new();
        tracker.track(TrackedEntity::experience_orb(1, 10.0, 64.0, 10.0, 16.0));

        let near = (12.0, 12.0);
        let far = (90.0, 90.0);
        assert!(tracker.is_visible_to(1, near.0, near.1));
        assert!(!tracker.is_visible_to(1, far.0, far.1));

        for y in [64.0, 40.0, 12.0, 0.0, -8.0] {
            tracker.update_position(1, 10.0, y, 10.0);
            assert!(
                tracker.is_visible_to(1, near.0, near.1),
                "near viewer lost the orb at y={y}, but it is an XZ test"
            );
            assert!(
                !tracker.is_visible_to(1, far.0, far.1),
                "far viewer gained the orb at y={y}, but it is an XZ test"
            );
        }
    }

    #[test]
    fn orbs_and_items_share_one_tracker_without_colliding() {
        let mut tracker = EntityTracker::new();
        for eid in 0..6 {
            if eid & 1 == 0 {
                tracker.track(TrackedEntity::item(eid, 0.0, 64.0, 0.0, 64.0));
            } else {
                tracker.track(TrackedEntity::experience_orb(eid, 0.0, 64.0, 0.0, 64.0));
            }
        }
        assert_eq!(tracker.len(), 6);

        tracker.untrack_many(&[1, 3, 5]);

        assert_eq!(tracker.len(), 3);
        for eid in [0, 2, 4] {
            assert_eq!(tracker.entities[&eid].entity_type, EntityType::Item);
        }
    }
}

#[cfg(test)]
mod range_tests {
    use super::*;

    #[test]
    fn the_helper_agrees_with_the_tracker() {
        let range = 64.0;
        let mut tracker = EntityTracker::new();

        let mut seed = 0x1234_5678_9ABC_DEF0u64;
        let mut next = || {
            seed ^= seed >> 12;
            seed ^= seed << 25;
            seed ^= seed >> 27;
            (seed.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
        };

        for eid in 0..500i32 {
            let x = next() * 400.0 - 200.0;
            let z = next() * 400.0 - 200.0;
            tracker.track(TrackedEntity::player(eid, Uuid::nil(), x, 64.0, z, range));

            for _ in 0..4 {
                let vx = next() * 400.0 - 200.0;
                let vz = next() * 400.0 - 200.0;
                assert_eq!(
                    tracker.is_visible_to(eid, vx, vz),
                    within_tracking_range(x, z, vx, vz, range),
                    "entity {eid} at ({x}, {z}) seen from ({vx}, {vz})"
                );
            }
        }
    }

    #[test]
    fn the_boundary_is_inclusive() {
        assert!(within_tracking_range(64.0, 0.0, 0.0, 0.0, 64.0));
        assert!(!within_tracking_range(64.01, 0.0, 0.0, 0.0, 64.0));
    }

    #[test]
    fn range_is_a_circle_not_a_square() {
        let range = 10.0;
        assert!(within_tracking_range(10.0, 0.0, 0.0, 0.0, range));
        assert!(
            !within_tracking_range(10.0, 10.0, 0.0, 0.0, range),
            "the diagonal is 14.1 away and must be out of range"
        );
        assert!(within_tracking_range(7.0, 7.0, 0.0, 0.0, range), "9.9 away");
    }

    #[test]
    fn height_is_ignored() {
        assert!(within_tracking_range(0.0, 0.0, 0.0, 0.0, 64.0));
    }

    #[test]
    fn negative_coordinates_measure_the_same_as_positive_ones() {
        assert_eq!(
            within_tracking_range(-30.0, -40.0, 0.0, 0.0, 50.0),
            within_tracking_range(30.0, 40.0, 0.0, 0.0, 50.0)
        );
        assert!(within_tracking_range(-30.0, -40.0, 0.0, 0.0, 50.0));
        assert!(!within_tracking_range(-30.0, -40.0, 0.0, 0.0, 49.9));
    }

    #[test]
    fn the_tracker_hides_untracked_entities_and_the_helper_cannot() {
        let tracker = EntityTracker::new();
        assert!(!tracker.is_visible_to(99, 0.0, 0.0), "never tracked");
        assert!(
            within_tracking_range(0.0, 0.0, 0.0, 0.0, 64.0),
            "distance alone says yes, because there is nothing to look up"
        );
    }
}
