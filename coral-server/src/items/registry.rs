use crate::items::{EmptyHand, UnknownItem, armor::ArmorSlot, ids::EMPTY_HAND};

use super::{Item, UseAction};
use std::{collections::HashMap, sync::Arc};

pub struct ItemRegistry {
    items: HashMap<i16, Arc<dyn Item>>,
}

impl ItemRegistry {
    pub fn new() -> Self {
        let mut items: HashMap<i16, Arc<dyn Item>> = HashMap::new(); // TODO: size it once all items are done

        items.insert(EMPTY_HAND, Arc::new(EmptyHand));
        for s in super::swords::all() {
            items.insert(s.id(), Arc::new(s));
        }
        for t in super::tools::all() {
            items.insert(t.id(), Arc::new(t));
        }
        for f in super::food::all() {
            items.insert(f.id(), Arc::new(f));
        }
        for a in super::armor::all() {
            items.insert(a.id(), Arc::new(a));
        }
        Self { items }
    }

    pub fn resolve(&self, item_id: i16) -> Arc<dyn Item> {
        self.get(item_id)
            .cloned()
            .unwrap_or_else(|| Arc::new(UnknownItem(item_id)))
    }

    pub fn get(&self, item_id: i16) -> Option<&Arc<dyn Item>> {
        self.items.get(&item_id)
    }

    pub fn attack_damage(&self, item_id: i16) -> f32 {
        self.get(item_id).map(|i| i.attack_damage()).unwrap_or(1.0)
    }

    pub fn food_value(&self, item_id: i16) -> Option<(i32, f32)> {
        self.get(item_id).and_then(|i| i.food_value())
    }

    pub fn max_durability(&self, item_id: i16) -> Option<i16> {
        self.get(item_id).and_then(|i| i.max_durability())
    }

    pub fn mining_speed(&self, item_id: i16, block_id: u8) -> f32 {
        self.get(item_id)
            .map(|i| i.mining_speed(block_id))
            .unwrap_or(1.0)
    }

    pub fn on_use(&self, item_id: i16) -> Option<UseAction> {
        self.get(item_id).and_then(|i| i.on_use())
    }

    pub fn armor_defense(&self, item_id: i16) -> Option<i32> {
        self.get(item_id).and_then(|i| i.armor_defense())
    }
    pub fn armor_slot(&self, item_id: i16) -> Option<ArmorSlot> {
        self.get(item_id).and_then(|i| i.armor_slot())
    }
}

#[cfg(test)]
mod resolve_tests {
    use super::*;
    use crate::items::ids::{ARROW, BOW, FISHING_ROD, SPLASH_POTION};

    #[test]
    fn an_unregistered_item_keeps_its_own_id() {
        let registry = ItemRegistry::new();
        for id in [BOW, ARROW, FISHING_ROD, SPLASH_POTION] {
            assert_eq!(
                registry.resolve(id).id(),
                id,
                "item {id} resolved to something else"
            );
        }
    }

    #[test]
    fn it_holds_for_every_id_the_registry_does_not_know() {
        let registry = ItemRegistry::new();
        for id in 0..=512i16 {
            assert_eq!(registry.resolve(id).id(), id, "item {id}");
        }
    }

    #[test]
    fn a_registered_item_is_still_the_registered_one() {
        let registry = ItemRegistry::new();
        let sword = registry.resolve(276);
        assert_eq!(sword.id(), 276);
        assert_eq!(
            sword.attack_damage(),
            7.0,
            "resolved to something without the sword's stats"
        );
    }

    #[test]
    fn an_empty_hand_is_still_an_empty_hand() {
        let registry = ItemRegistry::new();
        let hand = registry.resolve(EMPTY_HAND);
        assert_eq!(hand.id(), EMPTY_HAND);
        assert_eq!(hand.max_stack_size(), 1);
    }

    #[test]
    fn an_unknown_item_has_no_special_behaviour() {
        let registry = ItemRegistry::new();
        let bow = registry.resolve(BOW);
        assert!(bow.food_value().is_none());
        assert!(bow.tool_material().is_none());
        assert!(bow.armor_slot().is_none());
        assert_eq!(bow.attack_damage(), 1.0);
    }

    #[test]
    fn only_minus_one_is_the_empty_hand() {
        let registry = ItemRegistry::new();
        assert_eq!(registry.resolve(-2).id(), -2);
        assert_eq!(registry.resolve(EMPTY_HAND).id(), EMPTY_HAND);
    }
}
