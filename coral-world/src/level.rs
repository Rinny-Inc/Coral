use std::{
    io::{Read, Write},
    path::Path,
};

use flate2::{Compression, read::GzDecoder, write::GzEncoder};

use crate::nbt::{NbtReader, NbtTag};

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

pub async fn write_spawn_point(
    world_dir: &Path,
    x: i32,
    y: i32,
    z: i32,
    yaw: f32,
    pitch: f32,
) -> std::io::Result<()> {
    use flate2::Compression;
    use flate2::read::GzDecoder;
    use flate2::write::GzEncoder;
    use std::io::Read;
    use std::io::Write;

    let path = world_dir.join("level.dat");

    let compressed = tokio::fs::read(&path).await?;
    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut nbt_bytes = Vec::new();
    decoder.read_to_end(&mut nbt_bytes)?;

    let mut reader = crate::nbt::NbtReader::new(&nbt_bytes);
    let (root_name, mut root) = reader.read_named_root();

    if let Some(data) = root.get_mut("Data") {
        data.set("SpawnX", NbtTag::Int(x));
        data.set("SpawnY", NbtTag::Int(y));
        data.set("SpawnZ", NbtTag::Int(z));
        data.set("SpawnYaw", NbtTag::Float(yaw));
        data.set("SpawnPitch", NbtTag::Float(pitch));
    }

    let mut out = Vec::new();
    NbtTag::write_named_root(&root_name, &root, &mut out);

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&out)?;
    let compressed = encoder.finish()?;

    tokio::fs::write(&path, &compressed).await?;
    Ok(())
}

pub async fn read_spawn_point(world_dir: &Path) -> Option<(f64, f64, f64, f32, f32)> {
    let compressed = tokio::fs::read(world_dir.join("level.dat")).await.ok()?;
    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut nbt_bytes = Vec::new();
    decoder.read_to_end(&mut nbt_bytes).ok()?;

    let mut reader = NbtReader::new(&nbt_bytes);
    let (_, root) = reader.read_named_root();
    let data = root.get("Data")?;

    let x = data.get("SpawnX").and_then(|t| t.as_i32())? as f64 + 0.5;
    let y = data.get("SpawnY").and_then(|t| t.as_i32())? as f64 + 5.0; // FIXME: +5 is temp remove when command /setworldspawn is created
    let z = data.get("SpawnZ").and_then(|t| t.as_i32())? as f64 + 0.5;

    let yaw = data.get("SpawnYaw").and_then(|t| t.as_i32())? as f32;
    let pitch = data.get("SpawnPitch").and_then(|t| t.as_i32())? as f32;

    Some((x, y, z, yaw, pitch))
}
