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
        ((self / 360.0 * 256.0).floor() as i32 & 0xFF) as u8
    }
}

#[cfg(test)]
mod angle_tests {
    use super::AngleExt;

    #[test]
    fn the_cardinal_angles_pack_exactly() {
        assert_eq!(0.0f32.to_byte(), 0);
        assert_eq!(90.0f32.to_byte(), 64);
        assert_eq!(180.0f32.to_byte(), 128);
        assert_eq!(270.0f32.to_byte(), 192);
        assert_eq!((-90.0f32).to_byte(), 192);
        assert_eq!((-180.0f32).to_byte(), 128);
    }

    #[test]
    fn small_negative_angles_wrap_instead_of_rounding_to_zero() {
        assert_eq!((-1.0f32).to_byte(), 255);
        assert_eq!((-0.5f32).to_byte(), 255);
        assert_eq!((-1.40625f32).to_byte(), 255, "exactly one step below zero");
        assert_eq!((-1.5f32).to_byte(), 254);
    }

    #[test]
    fn packing_is_monotonic_across_zero() {
        let mut previous = (-10.0f32).to_byte() as i32 - 256;
        let mut degrees = -10.0f32;

        while degrees <= 10.0 {
            let packed = degrees.to_byte() as i32;
            let unwrapped = if packed > 128 { packed - 256 } else { packed };
            assert!(
                unwrapped >= previous,
                "{degrees} packed to {packed} after {previous}"
            );
            previous = unwrapped;
            degrees += 0.1;
        }
    }

    #[test]
    fn whole_turns_are_equivalent() {
        for base in [0.0f32, 37.0, 123.5, -64.25] {
            let packed = base.to_byte();
            assert_eq!((base + 360.0).to_byte(), packed, "base {base}");
            assert_eq!((base - 360.0).to_byte(), packed, "base {base}");
        }
    }
}
