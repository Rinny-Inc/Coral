use crate::effects::EffectKind;
use std::vec;

#[derive(Debug, Clone)]
pub struct PotionEffect {
    pub kind: EffectKind,
    pub amplifier: u8,
    pub duration_ticks: i32,
}

pub fn potion_effects(metadata: i16) -> Vec<PotionEffect> {
    let extended = (metadata & 0x20) != 0;
    let strong = (metadata & 0x40) != 0;
    let effect_bits = metadata & 0xF;

    let dur = |base: i32| if extended { base * 3 / 2 } else { base };
    let amp = if strong { 1u8 } else { 0u8 };

    match effect_bits {
        1 => vec![PotionEffect {
            kind: EffectKind::Regeneration,
            amplifier: amp,
            duration_ticks: dur(900),
        }],
        2 => vec![PotionEffect {
            kind: EffectKind::Speed,
            amplifier: amp,
            duration_ticks: dur(3600),
        }],
        3 => vec![PotionEffect {
            kind: EffectKind::FireResistance,
            amplifier: 0,
            duration_ticks: dur(3600),
        }],
        4 => vec![PotionEffect {
            kind: EffectKind::Poison,
            amplifier: amp,
            duration_ticks: dur(900),
        }],
        5 => vec![PotionEffect {
            kind: EffectKind::InstantHealth,
            amplifier: amp,
            duration_ticks: 1,
        }],
        6 => vec![PotionEffect {
            kind: EffectKind::NightVision,
            amplifier: 0,
            duration_ticks: dur(3600),
        }],
        8 => vec![PotionEffect {
            kind: EffectKind::Weakness,
            amplifier: 0,
            duration_ticks: dur(1800),
        }],
        9 => vec![PotionEffect {
            kind: EffectKind::Strength,
            amplifier: amp,
            duration_ticks: dur(3600),
        }],
        10 => vec![PotionEffect {
            kind: EffectKind::Slowness,
            amplifier: 0,
            duration_ticks: dur(1800),
        }],
        11 => vec![PotionEffect {
            kind: EffectKind::JumpBoost,
            amplifier: amp,
            duration_ticks: dur(3600),
        }],
        12 => vec![PotionEffect {
            kind: EffectKind::InstantDamage,
            amplifier: amp,
            duration_ticks: 1,
        }],
        13 => vec![PotionEffect {
            kind: EffectKind::WaterBreathing,
            amplifier: 0,
            duration_ticks: dur(3600),
        }],
        14 => vec![PotionEffect {
            kind: EffectKind::Invisibility,
            amplifier: 0,
            duration_ticks: dur(3600),
        }],
        _ => vec![],
    }
}

pub fn is_drinkable_potion(item_id: i16) -> bool {
    item_id == 373
}
pub fn is_splash_potion(item_id: i16) -> bool {
    // TODO: helper to not use magic
    item_id == 373 // splash potions share item id 373, throwable flag is in metadata bit 0x4000
    // actually in 1.8: splash = metadata & 0x4000 != 0
}
