use std::time::{SystemTime, UNIX_EPOCH};

pub struct XorShift {
    state: u64,
}

impl XorShift {
    #[inline]
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::new(0, 0))
            .as_nanos();
        Self { state: seed.wrapping_add(0x9E3779B97F4A7C15) as u64 }
    }

    #[inline]
    pub fn next(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    #[inline]
    pub fn random_in_range(&mut self, min: u64, max: u64) -> u64 {
        let range = max.wrapping_sub(min).wrapping_add(1);

        if range == 0 {
            return min;
        }
        min.wrapping_add(self.next() % range)
    }
}