use std::{
    io::{Read, Write},
    path::Path,
};

use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use uuid::Uuid;

use crate::nbt::{NbtReader, NbtTag};

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
}
impl Default for PlayerData {
    fn default() -> Self {
        Self {
            x: 0.5,
            y: 4.5,
            z: 0.5,
            yaw: 90.0,
            pitch: 0.0,
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
            gamemode: 0,
            inventory: vec![],
        }
    }
}

fn player_path(world_dir: &Path, uuid: &Uuid) -> std::path::PathBuf {
    world_dir.join("playerdata").join(format!("{}.dat", uuid))
}

pub async fn load_player_data(world_dir: &Path, uuid: &Uuid) -> Option<PlayerData> {
    let path = player_path(world_dir, uuid);
    let compressed = tokio::fs::read(&path).await.ok()?;

    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut nbt_bytes = Vec::new();
    decoder.read_to_end(&mut nbt_bytes).ok()?;

    let mut reader = NbtReader::new(&nbt_bytes);
    let (_, root) = reader.read_named_root();

    let pos = root.get("Pos").and_then(|t| t.as_list())?;
    let x = if let NbtTag::Double(v) = &pos[0] {
        *v
    } else {
        0.5
    };
    let y = if let NbtTag::Double(v) = &pos[1] {
        *v
    } else {
        4.5
    };
    let z = if let NbtTag::Double(v) = &pos[2] {
        *v
    } else {
        0.5
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

    let health = root
        .get("Health")
        .and_then(|t| {
            if let NbtTag::Float(v) = t {
                Some(*v)
            } else {
                None
            }
        })
        .unwrap_or(20.0);
    let food = root.get("foodLevel").and_then(|t| t.as_i32()).unwrap_or(20);
    let food_saturation = root
        .get("foodSaturationLevel")
        .and_then(|t| {
            if let NbtTag::Float(v) = t {
                Some(*v)
            } else {
                None
            }
        })
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

    let root = NbtTag::Compound(vec![
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
            NbtTag::Byte(data.gamemode as i8),
        ),
        ("Inventory".to_string(), NbtTag::List(10, inventory_list)),
    ]);

    let mut nbt_bytes = Vec::new();
    NbtTag::write_named_root("", &root, &mut nbt_bytes);

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(&nbt_bytes).is_err() {
        return;
    }
    let Ok(compressed) = encoder.finish() else {
        return;
    };

    let path = player_path(world_dir, uuid);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    tokio::fs::write(&path, &compressed).await.ok();
}
