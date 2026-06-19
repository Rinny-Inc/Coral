use std::collections::HashMap;

use crate::{
    blocks::Block,
    generator::FlatWorldGenerator,
    nbt::{NbtReader, NbtTag},
};

// Serialize a chunk to 1.8 Anvil NBT format
pub fn chunk_to_nbt(
    chunk_x: i32,
    chunk_z: i32,
    blocks: &HashMap<(i32, u8, i32), Block>,
    generator: &FlatWorldGenerator,
) -> Vec<u8> {
    // build 16 sections (y=0..256 in groups of 16)
    let mut sections = vec![];

    for section_y in 0..16u8 {
        let y_start = section_y as i32 * 16;
        let y_end = y_start + 16;

        // collect block data for this section
        let mut block_ids = vec![0u8; 4096];
        let mut block_data = vec![0u8; 2048];
        let mut has_blocks = false;

        for y in y_start..y_end {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    let wx = chunk_x * 16 + x;
                    let wz = chunk_z * 16 + z;
                    let wy = y as u8;

                    let block = blocks
                        .get(&(wx, wy, wz))
                        .cloned()
                        .unwrap_or_else(|| generator.get(wy));

                    if block.id != 0 {
                        has_blocks = true;
                    }

                    // YZX ordering
                    let local_y = (y - y_start) as usize;
                    let idx = local_y * 256 + z as usize * 16 + x as usize;
                    block_ids[idx] = block.id;

                    // metadata nibble
                    let nibble_idx = idx / 2;
                    if idx & 1 == 0 {
                        block_data[nibble_idx] =
                            (block_data[nibble_idx] & 0xF0) | (block.metadata & 0x0F);
                    } else {
                        block_data[nibble_idx] =
                            (block_data[nibble_idx] & 0x0F) | ((block.metadata & 0x0F) << 4);
                    }
                }
            }
        }

        if !has_blocks {
            continue;
        }

        let section = NbtTag::Compound(vec![
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
        ]);
        sections.push(section);
    }

    // biomes (all plains = 1)
    let biomes = vec![1u8; 256];

    let heightmap = vec![0i32; 256];

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
        ("TileEntities".to_string(), NbtTag::List(10, vec![])),
        ("TileTicks".to_string(), NbtTag::List(10, vec![])),
    ]);

    let root = NbtTag::Compound(vec![("Level".to_string(), level)]);

    let mut out = Vec::new();
    NbtTag::write_named_root("", &root, &mut out);
    out
}

pub fn nbt_to_blocks(nbt_data: &[u8], out: &mut HashMap<(i32, u8, i32), Block>) {
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
    let chunk_x = *chunk_x;
    let chunk_z = *chunk_z;

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
                    let metadata = if let Some(data) = block_data {
                        let nibble_idx = idx / 2;
                        if idx & 1 == 0 {
                            data[nibble_idx] & 0x0F
                        } else {
                            (data[nibble_idx] >> 4) & 0x0F
                        }
                    } else {
                        0
                    };

                    if block_id == 0 {
                        continue;
                    }

                    let wx = chunk_x * 16 + x as i32;
                    let wy = (y_start + local_y as i32) as u8;
                    let wz = chunk_z * 16 + z as i32;

                    out.insert((wx, wy, wz), Block::new(block_id, metadata));
                }
            }
        }
    }
}
