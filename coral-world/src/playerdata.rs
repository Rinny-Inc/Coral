use std::{
    io::{Read, Write},
    path::Path,
};

use coral_types::SavedEffect;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use uuid::Uuid;

use crate::nbt::{NbtReader, NbtTag};

pub const MAX_AIR_TICKS: i16 = 300;

#[derive(Debug, Clone)]
pub struct PlayerData {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub food: i32,
    pub food_saturation: f32,
    pub gamemode: u8,
    pub inventory: Vec<(i16, i16, u8, i16)>,
    pub held_slot: u8,
    pub xp_total: i32,
    pub bed_spawn: Option<(i32, i32, i32)>,
    pub is_flying: bool,
    pub fire_ticks: i16,
    pub air_ticks: i16,
    pub active_effects: Vec<SavedEffect>,
}
impl Default for PlayerData {
    fn default() -> Self {
        Self {
            x: 0.5,
            y: 4.5,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
            gamemode: 0,
            inventory: vec![],
            held_slot: 0,
            xp_total: 0,
            bed_spawn: None,
            is_flying: false,
            fire_ticks: 0,
            air_ticks: MAX_AIR_TICKS,
            active_effects: vec![],
        }
    }
}

fn player_path(world_dir: &Path, uuid: &Uuid) -> std::path::PathBuf {
    world_dir.join("playerdata").join(format!("{}.dat", uuid))
}

pub async fn load_player_data(world_dir: &Path, uuid: &Uuid) -> Option<PlayerData> {
    let path = player_path(world_dir, uuid);
    let compressed = tokio::fs::read(&path).await.ok()?;

    let root = tokio::task::spawn_blocking(move || {
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut nbt_bytes = Vec::new();
        decoder.read_to_end(&mut nbt_bytes).ok()?;
        Some(NbtReader::new(&nbt_bytes).read_named_root().1)
    })
    .await
    .ok()
    .flatten()?;

    let pos = root.get("Pos").and_then(|t| t.as_list())?;
    let (x, y, z) = if let (NbtTag::Double(x), NbtTag::Double(y), NbtTag::Double(z)) =
        (&pos[0], &pos[1], &pos[2])
    {
        (*x, *y, *z)
    } else {
        (0.5, 4.5, 0.5)
    };

    let rotation = root.get("Rotation").and_then(|t| t.as_list());
    let (yaw, pitch) = if let Some(r) = rotation {
        let yaw = if let NbtTag::Float(v) = &r[0] {
            *v
        } else {
            90.0
        };
        let pitch = if let NbtTag::Float(v) = &r[1] {
            *v
        } else {
            0.0
        };
        (yaw, pitch)
    } else {
        (90.0, 0.0)
    };

    let health = root.get("Health").and_then(|t| t.as_f32()).unwrap_or(20.0);
    let food = root.get("foodLevel").and_then(|t| t.as_i32()).unwrap_or(20);
    let food_saturation = root
        .get("foodSaturationLevel")
        .and_then(|t| t.as_f32())
        .unwrap_or(5.0);
    let gamemode = root
        .get("playerGameType")
        .and_then(|t| t.as_i32())
        .unwrap_or(0) as u8;

    let mut inventory = vec![];
    if let Some(inv_list) = root.get("Inventory").and_then(|t| t.as_list()) {
        for item in inv_list {
            let slot = item.get("Slot").and_then(|t| t.as_i8()).unwrap_or(0) as i16;
            let id = item.get("id").and_then(|t| t.as_i32()).unwrap_or(-1) as i16;
            let count = item.get("Count").and_then(|t| t.as_i8()).unwrap_or(0) as u8;
            let damage = item.get("Damage").and_then(|t| t.as_i16_val()).unwrap_or(0);
            inventory.push((slot, id, count, damage));
        }
    }

    let held_slot = root
        .get("SelectedItemSlot")
        .and_then(|t| t.as_i32())
        .filter(|s| (0..=8).contains(s))
        .unwrap_or(0) as u8;

    let is_flying = root
        .get("abilities")
        .and_then(|a| a.get("flying"))
        .and_then(|t| t.as_i8())
        .unwrap_or(0)
        != 0;

    let fire_ticks = root
        .get("Fire")
        .and_then(|t| t.as_i16_val())
        .unwrap_or(0)
        .max(0);

    let air_ticks = root
        .get("Air")
        .and_then(|t| t.as_i16_val())
        .unwrap_or(MAX_AIR_TICKS)
        .clamp(0, MAX_AIR_TICKS);

    let mut active_effects = vec![];
    if let Some(effect_list) = root.get("ActiveEffects").and_then(|t| t.as_list()) {
        for effect in effect_list {
            let Some(id) = effect.get("Id").and_then(|t| t.as_i32()) else {
                continue;
            };
            let Some(remaining) = effect.get("Duration").and_then(|t| t.as_i32()) else {
                continue;
            };
            if !(0..=255).contains(&id) || remaining <= 0 {
                continue;
            }
            let amplifier = effect
                .get("Amplifier")
                .and_then(|t| t.as_i32())
                .unwrap_or(0)
                .clamp(0, 255) as u8;
            active_effects.push((id as u8, amplifier, remaining));
        }
    }

    let xp_total = root.get("XpTotal").and_then(|t| t.as_i32()).unwrap_or(0);

    let bed_spawn = match (
        root.get("SpawnX").and_then(|t| t.as_i32()),
        root.get("SpawnY").and_then(|t| t.as_i32()),
        root.get("SpawnZ").and_then(|t| t.as_i32()),
    ) {
        (Some(x), Some(y), Some(z)) => Some((x, y, z)),
        _ => None,
    };

    Some(PlayerData {
        x,
        y,
        z,
        yaw,
        pitch,
        health,
        food,
        food_saturation,
        gamemode,
        inventory,
        held_slot,
        xp_total,
        bed_spawn,
        is_flying,
        fire_ticks,
        air_ticks,
        active_effects,
    })
}

pub async fn save_player_data(world_dir: &Path, uuid: &Uuid, data: &PlayerData) {
    let mut inventory_list = vec![];
    for (slot, id, count, damage) in &data.inventory {
        if *id == -1 {
            continue;
        }
        inventory_list.push(NbtTag::Compound(vec![
            ("Slot".to_string(), NbtTag::Byte(*slot as i8)),
            ("id".to_string(), NbtTag::Int(*id as i32)),
            ("Count".to_string(), NbtTag::Byte(*count as i8)),
            ("Damage".to_string(), NbtTag::Short(*damage)),
        ]));
    }

    let mut root_field = vec![
        (
            "Pos".to_string(),
            NbtTag::List(
                6,
                vec![
                    NbtTag::Double(data.x),
                    NbtTag::Double(data.y),
                    NbtTag::Double(data.z),
                ],
            ),
        ),
        (
            "Rotation".to_string(),
            NbtTag::List(5, vec![NbtTag::Float(data.yaw), NbtTag::Float(data.pitch)]),
        ),
        ("Health".to_string(), NbtTag::Float(data.health)),
        ("foodLevel".to_string(), NbtTag::Int(data.food)),
        (
            "foodSaturationLevel".to_string(),
            NbtTag::Float(data.food_saturation),
        ),
        (
            "playerGameType".to_string(),
            NbtTag::Int(data.gamemode as i32),
        ),
        ("Inventory".to_string(), NbtTag::List(10, inventory_list)),
        (
            "SelectedItemSlot".to_string(),
            NbtTag::Int(data.held_slot as i32),
        ),
        ("XpTotal".to_string(), NbtTag::Int(data.xp_total)),
        ("Fire".to_string(), NbtTag::Short(data.fire_ticks)),
        ("Air".to_string(), NbtTag::Short(data.air_ticks)),
        (
            "abilities".to_string(),
            NbtTag::Compound(vec![(
                "flying".to_string(),
                NbtTag::Byte(i8::from(data.is_flying)),
            )]),
        ),
    ];

    if !data.active_effects.is_empty() {
        let effects = data
            .active_effects
            .iter()
            .map(|(id, amplifier, remaining)| {
                NbtTag::Compound(vec![
                    ("Id".to_string(), NbtTag::Byte(*id as i8)),
                    ("Amplifier".to_string(), NbtTag::Byte(*amplifier as i8)),
                    ("Duration".to_string(), NbtTag::Int(*remaining)),
                ])
            })
            .collect();
        root_field.push(("ActiveEffects".to_string(), NbtTag::List(10, effects)));
    }

    if let Some((x, y, z)) = data.bed_spawn {
        root_field.push(("SpawnX".to_string(), NbtTag::Int(x)));
        root_field.push(("SpawnY".to_string(), NbtTag::Int(y)));
        root_field.push(("SpawnZ".to_string(), NbtTag::Int(z)));
    }

    let root = NbtTag::Compound(root_field);

    let compressed = tokio::task::spawn_blocking(move || {
        let mut nbt_bytes = Vec::new();
        NbtTag::write_named_root("", &root, &mut nbt_bytes);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&nbt_bytes).ok()?;
        encoder.finish().ok()
    })
    .await;

    let Ok(Some(compressed)) = compressed else {
        tracing::error!("[World] Could not encode playerdata for {uuid}");
        return;
    };

    let path = player_path(world_dir, uuid);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    tokio::fs::write(&path, &compressed).await.ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("coral-pdata-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        d
    }

    fn uuid() -> Uuid {
        Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef)
    }

    fn read_root(dir: &Path) -> NbtTag {
        let compressed = std::fs::read(player_path(dir, &uuid())).unwrap();
        let mut nbt_bytes = Vec::new();
        GzDecoder::new(&compressed[..])
            .read_to_end(&mut nbt_bytes)
            .unwrap();
        NbtReader::new(&nbt_bytes).read_named_root().1
    }

    fn write_root(dir: &Path, root: &NbtTag) {
        let mut out = Vec::new();
        NbtTag::write_named_root("", root, &mut out);
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&out).unwrap();
        std::fs::write(player_path(dir, &uuid()), enc.finish().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn gamemode_survives_a_round_trip() {
        let dir = tmp("gm");
        for gm in 0..=3u8 {
            let data = PlayerData {
                gamemode: gm,
                ..Default::default()
            };
            save_player_data(&dir, &uuid(), &data).await;
            let back = load_player_data(&dir, &uuid()).await.expect("saved data");
            assert_eq!(back.gamemode, gm, "gamemode {gm} did not survive a save");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn every_field_survives_a_round_trip() {
        let dir = tmp("all");
        let data = PlayerData {
            x: 128.5,
            y: 71.0,
            z: -33.5,
            yaw: 137.25,
            pitch: -42.5,
            health: 13.5,
            food: 17,
            food_saturation: 2.5,
            gamemode: 1,
            inventory: vec![(36, 278, 1, 12), (8, 1, 64, 0)],
            held_slot: 5,
            xp_total: 4271,
            bed_spawn: Some((-100, 64, 250)),
            is_flying: true,
            fire_ticks: 97,
            air_ticks: 142,
            active_effects: vec![(1, 0, 600), (16, 2, 3600)],
        };
        save_player_data(&dir, &uuid(), &data).await;
        let back = load_player_data(&dir, &uuid()).await.expect("saved data");

        assert_eq!((back.x, back.y, back.z), (data.x, data.y, data.z));
        assert_eq!((back.yaw, back.pitch), (data.yaw, data.pitch));
        assert_eq!(back.health, data.health);
        assert_eq!(back.food, data.food);
        assert_eq!(back.food_saturation, data.food_saturation);
        assert_eq!(back.gamemode, data.gamemode);
        assert_eq!(back.xp_total, data.xp_total);
        assert_eq!(back.bed_spawn, data.bed_spawn);
        assert_eq!(back.inventory, data.inventory);
        assert_eq!(back.held_slot, data.held_slot);
        assert_eq!(back.is_flying, data.is_flying);
        assert_eq!(back.fire_ticks, data.fire_ticks);
        assert_eq!(back.air_ticks, data.air_ticks);
        assert_eq!(back.active_effects, data.active_effects);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn the_previously_unsaved_fields_are_not_defaults_after_a_relog() {
        let dir = tmp("relog");
        let default = PlayerData::default();
        let data = PlayerData {
            held_slot: 7,
            is_flying: true,
            fire_ticks: 44,
            air_ticks: 11,
            active_effects: vec![(5, 1, 240)],
            ..Default::default()
        };
        assert_ne!(data.held_slot, default.held_slot);
        assert_ne!(data.is_flying, default.is_flying);
        assert_ne!(data.fire_ticks, default.fire_ticks);
        assert_ne!(data.air_ticks, default.air_ticks);
        assert_ne!(data.active_effects, default.active_effects);

        save_player_data(&dir, &uuid(), &data).await;
        let back = load_player_data(&dir, &uuid()).await.expect("saved data");

        assert_eq!(back.held_slot, 7, "held hotbar slot reset");
        assert!(back.is_flying, "flying state reset");
        assert_eq!(back.fire_ticks, 44, "fire ticks reset");
        assert_eq!(back.air_ticks, 11, "air ticks reset");
        assert_eq!(back.active_effects, vec![(5, 1, 240)], "effects reset");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn a_player_with_no_effects_writes_no_effects_key() {
        let dir = tmp("noeffects");
        save_player_data(&dir, &uuid(), &PlayerData::default()).await;

        let root = read_root(&dir);
        assert!(root.get("ActiveEffects").is_none());
        assert!(
            load_player_data(&dir, &uuid())
                .await
                .unwrap()
                .active_effects
                .is_empty()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn a_file_without_the_new_keys_loads_with_defaults() {
        let dir = tmp("oldfile");
        save_player_data(&dir, &uuid(), &PlayerData::default()).await;

        let mut root = read_root(&dir);
        for key in ["SelectedItemSlot", "Fire", "Air", "abilities"] {
            if let NbtTag::Compound(entries) = &mut root {
                entries.retain(|(k, _)| k != key);
            }
        }
        write_root(&dir, &root);

        let back = load_player_data(&dir, &uuid()).await.expect("loads anyway");
        assert_eq!(back.held_slot, 0);
        assert!(!back.is_flying);
        assert_eq!(back.fire_ticks, 0);
        assert_eq!(
            back.air_ticks, MAX_AIR_TICKS,
            "a file with no Air key means a full lungful, not zero"
        );
        assert!(back.active_effects.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn out_of_range_scalars_are_brought_back_into_range() {
        let dir = tmp("clamp");
        save_player_data(&dir, &uuid(), &PlayerData::default()).await;

        let mut root = read_root(&dir);
        root.set("SelectedItemSlot", NbtTag::Int(37));
        root.set("Air", NbtTag::Short(4000));
        root.set("Fire", NbtTag::Short(-20));
        write_root(&dir, &root);

        let back = load_player_data(&dir, &uuid()).await.unwrap();
        assert_eq!(back.held_slot, 0, "slot 37 would index past the hotbar");
        assert_eq!(
            back.air_ticks, MAX_AIR_TICKS,
            "air above the ceiling never ticks back down to it"
        );
        assert_eq!(back.fire_ticks, 0, "negative fire would burn forever");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn unusable_effect_entries_are_dropped_not_restored() {
        let dir = tmp("badeffects");
        save_player_data(&dir, &uuid(), &PlayerData::default()).await;

        let mut root = read_root(&dir);
        root.set(
            "ActiveEffects",
            NbtTag::List(
                10,
                vec![
                    NbtTag::Compound(vec![
                        ("Id".to_string(), NbtTag::Byte(1)),
                        ("Amplifier".to_string(), NbtTag::Byte(0)),
                        ("Duration".to_string(), NbtTag::Int(0)),
                    ]),
                    NbtTag::Compound(vec![
                        ("Id".to_string(), NbtTag::Byte(2)),
                        ("Amplifier".to_string(), NbtTag::Byte(0)),
                        ("Duration".to_string(), NbtTag::Int(-500)),
                    ]),
                    NbtTag::Compound(vec![("Id".to_string(), NbtTag::Byte(3))]),
                    NbtTag::Compound(vec![("Duration".to_string(), NbtTag::Int(200))]),
                    NbtTag::Compound(vec![
                        ("Id".to_string(), NbtTag::Byte(9)),
                        ("Duration".to_string(), NbtTag::Int(300)),
                    ]),
                ],
            ),
        );
        write_root(&dir, &root);

        let back = load_player_data(&dir, &uuid()).await.unwrap();
        assert_eq!(
            back.active_effects,
            vec![(9, 0, 300)],
            "only the usable entry should survive, amplifier defaulted to 0"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn effect_amplifiers_round_trip_across_the_byte_range() {
        let dir = tmp("amp");
        for amplifier in [0u8, 1, 4, 127] {
            let data = PlayerData {
                active_effects: vec![(1, amplifier, 400)],
                ..Default::default()
            };
            save_player_data(&dir, &uuid(), &data).await;
            let back = load_player_data(&dir, &uuid()).await.unwrap();
            assert_eq!(
                back.active_effects,
                vec![(1, amplifier, 400)],
                "amplifier {amplifier} did not survive"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn a_player_with_no_bed_loads_without_one() {
        let dir = tmp("nobed");
        save_player_data(&dir, &uuid(), &PlayerData::default()).await;
        assert_eq!(
            load_player_data(&dir, &uuid()).await.unwrap().bed_spawn,
            None
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn a_legacy_byte_gamemode_still_loads() {
        let dir = tmp("legacy");
        save_player_data(&dir, &uuid(), &PlayerData::default()).await;

        let path = player_path(&dir, &uuid());
        let compressed = std::fs::read(&path).unwrap();
        let mut nbt_bytes = Vec::new();
        GzDecoder::new(&compressed[..])
            .read_to_end(&mut nbt_bytes)
            .unwrap();
        let (name, mut root) = NbtReader::new(&nbt_bytes).read_named_root();
        root.set("playerGameType", NbtTag::Byte(1));
        let mut out = Vec::new();
        NbtTag::write_named_root(&name, &root, &mut out);
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&out).unwrap();
        std::fs::write(&path, enc.finish().unwrap()).unwrap();

        assert_eq!(load_player_data(&dir, &uuid()).await.unwrap().gamemode, 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
