pub mod living;
pub mod player;

pub trait Tickable {
    fn tick();
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
}

impl EntityTrait for Entity {
    fn id(&self) -> u128 {
        self.id
    }
}