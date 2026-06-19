#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub width: f64,
    pub height: f64,
}
impl BoundingBox {
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn intersects(
        &self,
        self_x: f64,
        self_y: f64,
        self_z: f64,
        other: &BoundingBox,
        other_x: f64,
        other_y: f64,
        other_z: f64,
    ) -> bool {
        let half_self = self.width / 2.0;
        let half_other = other.width / 2.0;

        let dx = (self_x - other_x).abs();
        let dy = (self_y - other_y).abs();
        let dz = (self_z - other_z).abs();

        dx < half_self + half_other
            && dy < self.height.max(other.height)
            && dz < half_self + half_other
    }

    pub fn contains(&self, box_x: f64, box_y: f64, box_z: f64, px: f64, py: f64, pz: f64) -> bool {
        let half = self.width / 2.0;
        px >= box_x - half
            && px <= box_x + half
            && py >= box_y
            && py <= box_y + self.height
            && pz >= box_z - half
            && pz <= box_z + half
    }
}

pub struct EntityBounds;
impl EntityBounds {
    pub fn player() -> BoundingBox {
        BoundingBox::new(0.6, 1.8)
    }
    pub fn player_sneaking() -> BoundingBox {
        BoundingBox::new(0.6, 1.65)
    }

    pub fn item() -> BoundingBox {
        BoundingBox::new(0.25, 0.25)
    }

    pub fn experience_orb() -> BoundingBox {
        BoundingBox::new(0.5, 0.5)
    }

    pub fn zombie() -> BoundingBox {
        BoundingBox::new(0.6, 1.95)
    }

    pub fn creeper() -> BoundingBox {
        BoundingBox::new(0.6, 1.7)
    }

    pub fn skeleton() -> BoundingBox {
        BoundingBox::new(0.6, 1.99)
    }

    pub fn spider() -> BoundingBox {
        BoundingBox::new(1.4, 0.9)
    }

    pub fn cow() -> BoundingBox {
        BoundingBox::new(0.9, 1.4)
    }

    pub fn pig() -> BoundingBox {
        BoundingBox::new(0.9, 0.9)
    }

    pub fn chicken() -> BoundingBox {
        BoundingBox::new(0.4, 0.7)
    }

    pub fn sheep() -> BoundingBox {
        BoundingBox::new(0.9, 1.3)
    }

    pub fn projectile() -> BoundingBox {
        BoundingBox::new(0.5, 0.5)
    }
}
