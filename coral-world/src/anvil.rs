use std::collections::HashMap;

use coral_protocol::packets::play::entity::TileEntity;

use crate::{
    blocks::Block,
    generator::FlatWorldGenerator,
    nbt::{NbtReader, NbtTag},
};

pub fn chunk_to_nbt(
    chunk_x: i32,
    chunk_z: i32,
    blocks: &HashMap<(i32, u8, i32), Block>,
    generator: &FlatWorldGenerator,
    tile_entities: Vec<NbtTag>,
    is_new_chunk: bool,
) -> Vec<u8> {
    let mut sections = vec![];

    for section_y in 0..16u8 {
        let y_start = section_y as i32 * 16;

        let mut block_ids = vec![0u8; 4096];
        let mut block_data = vec![0u8; 2048];
        let mut has_nonair = false;

        for local_y in 0..16usize {
            let wy = (y_start + local_y as i32) as u8;
            for z in 0..16usize {
                for x in 0..16usize {
                    let wx = chunk_x * 16 + x as i32;
                    let wz = chunk_z * 16 + z as i32;

                    // HashMap first (includes explicit air overrides), then generator
                    let block = blocks.get(&(wx, wy, wz)).cloned().unwrap_or_else(|| {
                        if is_new_chunk {
                            generator.get(wy)
                        } else {
                            Block::air()
                        }
                    });

                    if block.id != 0 {
                        has_nonair = true;
                    }

                    // YZX ordering
                    let idx = local_y * 256 + z * 16 + x;
                    block_ids[idx] = block.id;

                    let ni = idx / 2;
                    if idx & 1 == 0 {
                        block_data[ni] = (block_data[ni] & 0xF0) | (block.metadata & 0x0F);
                    } else {
                        block_data[ni] = (block_data[ni] & 0x0F) | ((block.metadata & 0x0F) << 4);
                    }
                }
            }
        }

        // always include section 0 (bedrock layer) even if all air above
        // skip higher sections that are truly empty
        if !has_nonair && section_y > 0 {
            continue;
        }

        sections.push(NbtTag::Compound(vec![
            ("Y".to_string(), NbtTag::Byte(section_y as i8)),
            ("Blocks".to_string(), NbtTag::ByteArray(block_ids)),
            ("Data".to_string(), NbtTag::ByteArray(block_data)),
            (
                "BlockLight".to_string(),
                NbtTag::ByteArray(vec![0xFFu8; 2048]),
            ),
            (
                "SkyLight".to_string(),
                NbtTag::ByteArray(vec![0xFFu8; 2048]),
            ),
        ]));
    }

    let biomes = vec![1u8; 256]; // plains

    let mut heightmap = vec![0i32; 256];
    for z in 0..16usize {
        for x in 0..16usize {
            let wx = chunk_x * 16 + x as i32;
            let wz = chunk_z * 16 + z as i32;
            let mut h = 0i32;
            for y in (0u8..u8::MAX).rev() {
                let b = blocks
                    .get(&(wx, y, wz))
                    .cloned()
                    .unwrap_or_else(|| generator.get(y));
                if b.id != 0 {
                    h = y as i32 + 1;
                    break;
                }
            }
            heightmap[z * 16 + x] = h;
        }
    }

    let level = NbtTag::Compound(vec![
        ("xPos".to_string(), NbtTag::Int(chunk_x)),
        ("zPos".to_string(), NbtTag::Int(chunk_z)),
        ("LastUpdate".to_string(), NbtTag::Long(0)),
        ("LightPopulated".to_string(), NbtTag::Byte(1)),
        ("TerrainPopulated".to_string(), NbtTag::Byte(1)),
        ("V".to_string(), NbtTag::Byte(1)),
        ("InhabitedTime".to_string(), NbtTag::Long(0)),
        ("Biomes".to_string(), NbtTag::ByteArray(biomes)),
        ("HeightMap".to_string(), NbtTag::IntArray(heightmap)),
        ("Sections".to_string(), NbtTag::List(10, sections)),
        ("Entities".to_string(), NbtTag::List(10, vec![])),
        ("TileEntities".to_string(), NbtTag::List(10, tile_entities)),
        ("TileTicks".to_string(), NbtTag::List(10, vec![])),
    ]);

    let root = NbtTag::Compound(vec![("Level".to_string(), level)]);
    let mut out = Vec::new();
    NbtTag::write_named_root("", &root, &mut out);
    out
}

#[derive(Debug)]
pub struct RawTileEntity {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub id: String,
    pub compound: NbtTag,
}

pub fn tile_entity_to_nbt(x: i32, y: i32, z: i32, tile: &TileEntity) -> Option<NbtTag> {
    match tile {
        TileEntity::Sign { lines } => Some(NbtTag::Compound(vec![
            ("id".to_string(), NbtTag::String("Sign".to_string())),
            ("x".to_string(), NbtTag::Int(x)),
            ("y".to_string(), NbtTag::Int(y)),
            ("z".to_string(), NbtTag::Int(z)),
            (
                "Text1".to_string(),
                NbtTag::String(format!(r#"{{"text":"{}"}}"#, escape_json(&lines[0]))),
            ),
            (
                "Text2".to_string(),
                NbtTag::String(format!(r#"{{"text":"{}"}}"#, escape_json(&lines[1]))),
            ),
            (
                "Text3".to_string(),
                NbtTag::String(format!(r#"{{"text":"{}"}}"#, escape_json(&lines[2]))),
            ),
            (
                "Text4".to_string(),
                NbtTag::String(format!(r#"{{"text":"{}"}}"#, escape_json(&lines[3]))),
            ),
        ])),
        TileEntity::Chest { items } => {
            let item_list: Vec<NbtTag> = items
                .iter()
                .enumerate()
                .filter_map(|(slot, item)| {
                    let s = item.as_ref()?;
                    Some(NbtTag::Compound(vec![
                        ("Slot".to_string(), NbtTag::Byte(slot as i8)),
                        ("id".to_string(), NbtTag::Int(s.item_id as i32)),
                        ("Count".to_string(), NbtTag::Byte(s.count as i8)),
                        ("Damage".to_string(), NbtTag::Short(s.metadata)),
                    ]))
                })
                .collect();

            Some(NbtTag::Compound(vec![
                ("id".to_string(), NbtTag::String("Chest".to_string())),
                ("x".to_string(), NbtTag::Int(x)),
                ("y".to_string(), NbtTag::Int(y)),
                ("z".to_string(), NbtTag::Int(z)),
                ("Items".to_string(), NbtTag::List(10, item_list)),
            ]))
        }
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn nbt_to_tile_entities(nbt_data: &[u8]) -> Vec<RawTileEntity> {
    let mut reader = NbtReader::new(nbt_data);
    let (_, root) = reader.read_named_root();
    let mut out = vec![];

    let Some(level) = root.get("Level") else {
        return out;
    };
    let Some(list) = level.get("TileEntities").and_then(|t| t.as_list()) else {
        return out;
    };

    for entry in list {
        let Some(id) = entry.get("id").and_then(|t| t.as_string()) else {
            continue;
        };
        let x = entry.get("x").and_then(|t| t.as_i32()).unwrap_or(0);
        let y = entry.get("y").and_then(|t| t.as_i32()).unwrap_or(0);
        let z = entry.get("z").and_then(|t| t.as_i32()).unwrap_or(0);

        out.push(RawTileEntity {
            x,
            y,
            z,
            id: id.to_string(),
            compound: entry.clone(),
        });
    }

    out
}

pub fn nbt_to_blocks(
    nbt_data: &[u8],
    out: &mut HashMap<(i32, u8, i32), Block>,
    generator: &FlatWorldGenerator,
) {
    let mut reader = NbtReader::new(nbt_data);
    let (_, root) = reader.read_named_root();

    let Some(level) = root.get("Level") else {
        return;
    };
    let Some(NbtTag::Int(chunk_x)) = level.get("xPos") else {
        return;
    };
    let Some(NbtTag::Int(chunk_z)) = level.get("zPos") else {
        return;
    };
    let (chunk_x, chunk_z) = (*chunk_x, *chunk_z);

    let Some(sections) = level.get("Sections").and_then(|s| s.as_list()) else {
        return;
    };

    for section in sections {
        let Some(section_y) = section.get("Y").and_then(|v| v.as_i8()) else {
            continue;
        };
        let Some(block_ids) = section.get("Blocks").and_then(|v| v.as_byte_array()) else {
            continue;
        };
        let block_data = section.get("Data").and_then(|v| v.as_byte_array());

        let y_start = section_y as i32 * 16;

        for local_y in 0..16usize {
            let wy = (y_start + local_y as i32) as u8;
            let gen_block = generator.get(wy);

            for z in 0..16usize {
                for x in 0..16usize {
                    let idx = local_y * 256 + z * 16 + x;
                    let block_id = block_ids[idx];
                    let metadata = block_data
                        .map(|d| {
                            let ni = idx / 2;
                            if idx & 1 == 0 {
                                d[ni] & 0x0F
                            } else {
                                (d[ni] >> 4) & 0x0F
                            }
                        })
                        .unwrap_or(0);

                    // skip if matches generator — no need to store
                    if block_id == gen_block.id && metadata == gen_block.metadata {
                        continue;
                    }

                    let wx = chunk_x * 16 + x as i32;
                    let wz = chunk_z * 16 + z as i32;
                    out.insert((wx, wy, wz), Block::new(block_id, metadata));
                }
            }
        }
    }
}

pub fn nbt_to_blocks_raw(nbt_data: &[u8], out: &mut HashMap<(i32, u8, i32), Block>) {
    let mut reader = NbtReader::new(nbt_data);
    let (_, root) = reader.read_named_root();
    let Some(level) = root.get("Level") else {
        return;
    };
    let Some(NbtTag::Int(chunk_x)) = level.get("xPos") else {
        return;
    };
    let Some(NbtTag::Int(chunk_z)) = level.get("zPos") else {
        return;
    };
    let (chunk_x, chunk_z) = (*chunk_x, *chunk_z);
    let Some(sections) = level.get("Sections").and_then(|s| s.as_list()) else {
        return;
    };

    for section in sections {
        let Some(section_y) = section.get("Y").and_then(|v| v.as_i8()) else {
            continue;
        };
        let Some(block_ids) = section.get("Blocks").and_then(|v| v.as_byte_array()) else {
            continue;
        };
        let block_data = section.get("Data").and_then(|v| v.as_byte_array());
        let y_start = section_y as i32 * 16;

        for local_y in 0..16usize {
            for z in 0..16usize {
                for x in 0..16usize {
                    let idx = local_y * 256 + z * 16 + x;
                    let block_id = block_ids[idx];
                    if block_id == 0 {
                        continue;
                    }
                    let metadata = block_data
                        .map(|d| {
                            let ni = idx / 2;
                            if idx & 1 == 0 {
                                d[ni] & 0x0F
                            } else {
                                (d[ni] >> 4) & 0x0F
                            }
                        })
                        .unwrap_or(0);
                    let wx = chunk_x * 16 + x as i32;
                    let wy = (y_start + local_y as i32) as u8;
                    let wz = chunk_z * 16 + z as i32;
                    out.insert((wx, wy, wz), Block::new(block_id, metadata));
                }
            }
        }
    }
}
