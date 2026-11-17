use super::Item;
use coral_types::ids::items;

pub struct FoodItem {
    pub id: i16,
    pub hunger: i32,
    pub saturation: f32,
}

impl Item for FoodItem {
    fn id(&self) -> i16 {
        self.id
    }
    fn food_value(&self) -> Option<(i32, f32)> {
        Some((self.hunger, self.saturation))
    }
    fn max_stack_size(&self) -> u8 {
        64
    }
}

pub fn all() -> Vec<FoodItem> {
    vec![
        FoodItem {
            id: items::BREAD,
            hunger: 5,
            saturation: 6.0,
        },
        FoodItem {
            id: items::APPLE,
            hunger: 4,
            saturation: 2.4,
        },
        FoodItem {
            id: items::GOLDEN_APPLE,
            hunger: 10,
            saturation: 12.0,
        },
        FoodItem {
            id: items::PORKCHOP,
            hunger: 3,
            saturation: 1.8,
        },
        FoodItem {
            id: items::COOKED_PORKCHOP,
            hunger: 8,
            saturation: 12.8,
        },
        FoodItem {
            id: items::BEEF,
            hunger: 3,
            saturation: 1.8,
        },
        FoodItem {
            id: items::COOKED_BEEF,
            hunger: 8,
            saturation: 12.8,
        },
        FoodItem {
            id: items::CHICKEN,
            hunger: 2,
            saturation: 1.2,
        },
        FoodItem {
            id: items::COOKED_CHICKEN,
            hunger: 6,
            saturation: 7.2,
        },
        FoodItem {
            id: items::FISH,
            hunger: 2,
            saturation: 0.2,
        },
        FoodItem {
            id: items::COOKED_FISH,
            hunger: 5,
            saturation: 6.0,
        },
        FoodItem {
            id: items::COOKIE,
            hunger: 6,
            saturation: 7.2,
        },
        FoodItem {
            id: items::CARROT,
            hunger: 1,
            saturation: 0.6,
        },
        FoodItem {
            id: items::POTATO,
            hunger: 1,
            saturation: 0.6,
        },
        FoodItem {
            id: items::BAKED_POTATO,
            hunger: 6,
            saturation: 7.2,
        },
        FoodItem {
            id: items::PUMPKIN_PIE,
            hunger: 6,
            saturation: 7.2,
        },
        FoodItem {
            id: items::ROTTEN_FLESH,
            hunger: 3,
            saturation: 1.8,
        },
        FoodItem {
            id: items::MUTTON,
            hunger: 1,
            saturation: 0.6,
        },
        FoodItem {
            id: items::COOKED_MUTTON,
            hunger: 5,
            saturation: 6.0,
        },
    ]
}
