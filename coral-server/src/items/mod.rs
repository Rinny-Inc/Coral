pub mod drops;
pub mod food;
pub mod registry;
pub mod swords;
pub mod tools;

pub use registry::ItemRegistry;

pub trait Item: Send + Sync {
    fn id(&self) -> i16;

    fn attack_damage(&self) -> f32 {
        1.0
    }

    fn food_value(&self) -> Option<(i32, f32)> {
        None
    }

    fn max_stack_size(&self) -> u8 {
        64
    }

    fn max_durability(&self) -> Option<i16> {
        None
    }

    fn durability_cost_on_break(&self) -> i16 {
        1
    }

    fn durability_cost_on_attack(&self) -> i16 {
        2
    }

    fn mining_speed(&self, _block_id: u8) -> f32 {
        1.0
    }

    fn on_use(&self) -> Option<UseAction> {
        if self.food_value().is_some() {
            Some(UseAction::Eat)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UseAction {
    Eat,
    Drink,
    Bow,
    Block,
}
