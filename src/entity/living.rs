use super::{Entity, EntityTrait};

pub trait LivingTrait: EntityTrait {
    fn health(&self) -> f32;
    fn name(&self) -> &str;
}

pub struct Living {
    health: u8
}