use super::Item;

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
            id: 297,
            hunger: 5,
            saturation: 6.0,
        },
        FoodItem {
            id: 260,
            hunger: 4,
            saturation: 2.4,
        },
        FoodItem {
            id: 322,
            hunger: 10,
            saturation: 12.0,
        },
        FoodItem {
            id: 319,
            hunger: 3,
            saturation: 1.8,
        },
        FoodItem {
            id: 320,
            hunger: 8,
            saturation: 12.8,
        },
        FoodItem {
            id: 363,
            hunger: 3,
            saturation: 1.8,
        },
        FoodItem {
            id: 364,
            hunger: 8,
            saturation: 12.8,
        },
        FoodItem {
            id: 365,
            hunger: 2,
            saturation: 1.2,
        },
        FoodItem {
            id: 366,
            hunger: 6,
            saturation: 7.2,
        },
        FoodItem {
            id: 349,
            hunger: 2,
            saturation: 0.2,
        },
        FoodItem {
            id: 350,
            hunger: 5,
            saturation: 6.0,
        },
        FoodItem {
            id: 357,
            hunger: 6,
            saturation: 7.2,
        },
        FoodItem {
            id: 391,
            hunger: 1,
            saturation: 0.6,
        },
        FoodItem {
            id: 392,
            hunger: 1,
            saturation: 0.6,
        },
        FoodItem {
            id: 393,
            hunger: 6,
            saturation: 7.2,
        },
        FoodItem {
            id: 400,
            hunger: 6,
            saturation: 7.2,
        },
        FoodItem {
            id: 367,
            hunger: 3,
            saturation: 1.8,
        },
        FoodItem {
            id: 423,
            hunger: 1,
            saturation: 0.6,
        },
        FoodItem {
            id: 424,
            hunger: 5,
            saturation: 6.0,
        },
    ]
}
