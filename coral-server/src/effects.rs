#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EffectKind {
    Speed = 1,
    Slowness,
    Haste,
    MiningFatigue,
    Strength,
    InstantHealth,
    InstantDamage,
    JumpBoost,
    Nausea,
    Regeneration,
    Resistance,
    FireResistance,
    WaterBreathing,
    Invisibility,
    Blindness,
    NightVision,
    Hunger,
    Weakness,
    Poison,
    Wither,
    HealthBoost,
    Absorption,
    Saturation,
}
impl EffectKind {
    pub fn id(&self) -> u8 {
        self.clone() as u8
    }

    /// Inverse of [`Self::id`]
    ///
    /// Splash potions send `kind as u8` over the channel
    pub fn from_id(id: u8) -> Option<Self> {
        Some(match id {
            1 => Self::Speed,
            2 => Self::Slowness,
            3 => Self::Haste,
            4 => Self::MiningFatigue,
            5 => Self::Strength,
            6 => Self::InstantHealth,
            7 => Self::InstantDamage,
            8 => Self::JumpBoost,
            9 => Self::Nausea,
            10 => Self::Regeneration,
            11 => Self::Resistance,
            12 => Self::FireResistance,
            13 => Self::WaterBreathing,
            14 => Self::Invisibility,
            15 => Self::Blindness,
            16 => Self::NightVision,
            17 => Self::Hunger,
            18 => Self::Weakness,
            19 => Self::Poison,
            20 => Self::Wither,
            21 => Self::HealthBoost,
            22 => Self::Absorption,
            23 => Self::Saturation,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ActiveEffect {
    pub kind: EffectKind,
    pub amplifier: u8,
    pub duration_ticks: i32,
    pub remaining_ticks: i32,
}
impl ActiveEffect {
    pub fn new(kind: EffectKind, amplifier: u8, duration_ticks: i32) -> Self {
        Self {
            kind,
            amplifier,
            duration_ticks,
            remaining_ticks: duration_ticks,
        }
    }
}

#[cfg(test)]
mod id_tests {
    use super::*;

    #[test]
    fn ids_round_trip_through_the_channel_encoding() {
        for kind in [
            EffectKind::Speed,
            EffectKind::Slowness,
            EffectKind::Strength,
            EffectKind::InstantHealth,
            EffectKind::Regeneration,
            EffectKind::Resistance,
            EffectKind::FireResistance,
            EffectKind::Poison,
            EffectKind::Wither,
            EffectKind::Saturation,
        ] {
            let id = kind.id();
            assert_eq!(EffectKind::from_id(id), Some(kind.clone()), "id {id}");
        }
    }

    #[test]
    fn the_ids_are_the_vanilla_ones() {
        assert_eq!(EffectKind::Speed.id(), 1);
        assert_eq!(EffectKind::Strength.id(), 5);
        assert_eq!(EffectKind::Regeneration.id(), 10);
        assert_eq!(EffectKind::Poison.id(), 19);
        assert_eq!(EffectKind::Saturation.id(), 23);
    }

    #[test]
    fn unknown_ids_are_rejected() {
        assert_eq!(EffectKind::from_id(0), None);
        assert_eq!(EffectKind::from_id(24), None);
        assert_eq!(EffectKind::from_id(255), None);
    }
}
