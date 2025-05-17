pub mod living;
pub mod player;

pub trait Tickable {
    fn tick();
    fn tick_rate() -> u8;
}

pub trait EntityTrait {
    fn id(&self) -> u128;
}

pub struct Entity {
    id: u128
}

impl Tickable for Entity {
    fn tick() {
        // TODO
    }
    fn tick_rate() -> u8 {
        3
    }
}

impl EntityTrait for Entity {
    fn id(&self) -> u128 {
        self.id
    }
}