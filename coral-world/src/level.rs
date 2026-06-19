use std::{io::Write, path::Path};

use flate2::{Compression, write::GzEncoder};

use crate::nbt::NbtTag;

pub fn write_level_dat(world_dir: &Path, world_name: &str) {
    let data = NbtTag::Compound(vec![(
        "Data".to_string(),
        NbtTag::Compound(vec![
            (
                "LevelName".to_string(),
                NbtTag::String(world_name.to_string()),
            ),
            ("version".to_string(), NbtTag::Int(19133)), // Anvil
            ("DataVersion".to_string(), NbtTag::Int(0)),
            ("RandomSeed".to_string(), NbtTag::Long(0)),
            ("SpawnX".to_string(), NbtTag::Int(0)),
            ("SpawnY".to_string(), NbtTag::Int(0)),
            ("SpawnZ".to_string(), NbtTag::Int(0)),
            ("SpawnYaw".to_string(), NbtTag::Float(0.0)),
            ("SpawnPitch".to_string(), NbtTag::Float(90.0)),
            ("Time".to_string(), NbtTag::Long(0)),
            ("DayTime".to_string(), NbtTag::Long(6000)),
            ("GameType".to_string(), NbtTag::Int(0)),
            ("Difficulty".to_string(), NbtTag::Byte(1)),
            ("allowCommands".to_string(), NbtTag::Byte(1)),
            ("hardcore".to_string(), NbtTag::Byte(0)),
        ]),
    )]);

    let mut nbt_bytes = Vec::new();
    NbtTag::write_named_root("", &data, &mut nbt_bytes);

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&nbt_bytes).unwrap();
    let compressed = encoder.finish().unwrap();

    std::fs::create_dir_all(world_dir).ok();
    std::fs::write(world_dir.join("level.dat"), compressed).ok();
}
