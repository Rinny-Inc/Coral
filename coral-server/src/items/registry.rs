use super::{Item, UseAction};
use std::{collections::HashMap, sync::Arc};

pub struct ItemRegistry {
    items: HashMap<i16, Arc<dyn Item>>,
}

impl ItemRegistry {
    pub fn new() -> Self {
        let mut items: HashMap<i16, Arc<dyn Item>> = HashMap::new(); // TODO: size it once all items are done

        for s in super::swords::all() {
            items.insert(s.id(), Arc::new(s));
        }
        for t in super::tools::all() {
            items.insert(t.id(), Arc::new(t));
        }
        for f in super::food::all() {
            items.insert(f.id(), Arc::new(f));
        }
        Self { items }
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
}
