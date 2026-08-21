use std::time::Duration;

pub trait TicksExt {
    /// Convert a count of Minecraft ticks (20/sec, 50ms each) into a Duration.
    fn from_ticks(ticks: u64) -> Duration;

    /// Convert this Duration into a tick count, rounding down.
    fn as_ticks(&self) -> u64;
}
impl TicksExt for Duration {
    fn from_ticks(ticks: u64) -> Duration {
        Duration::from_millis(ticks * 50)
    }

    fn as_ticks(&self) -> u64 {
        self.as_millis() as u64 / 50
    }
}

pub trait AngleExt {
    /// Convert an angle in degrees to packed rotation byte
    fn to_byte(self) -> u8;
}
impl AngleExt for f32 {
    fn to_byte(self) -> u8 {
        // old -> ((degrees * 256.0 / 360.0) as i32).rem_euclid(256) as u8
        ((self / 360.0 * 256.0) as i32 & 0xFF) as u8
    }
}
