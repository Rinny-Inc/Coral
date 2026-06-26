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
