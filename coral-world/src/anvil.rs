use std::collections::HashMap;

use coral_protocol::packets::play::{entity::TileEntity, inventory::ItemStack};

use crate::{
    blocks::Block,
    generator::WorldGenerator,
    nbt::{NbtReader, NbtTag},
};

pub fn chunk_to_nbt(
    chunk_x: i32,
    chunk_z: i32,
    blocks: &HashMap<(i32, u8, i32), Block>,
    generator: &dyn WorldGenerator,
    tile_entities: Vec<NbtTag>,
    is_new_chunk: bool,
) -> Vec<u8> {
    const VOLUME: usize = 16 * 16 * 256;
    #[inline]
    fn index(x: i32, y: u8, z: i32) -> usize {
        ((y as usize) << 8) | (((z & 15) as usize) << 4) | ((x & 15) as usize)
    }

    let mut dense = vec![Block::air(); VOLUME];

    if is_new_chunk && !generator.is_empty() {
        let mut i = 0usize;
        for y in 0..255u8 {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    dense[i] = generator.block_at(chunk_x * 16 + x, y, chunk_z * 16 + z);
                    i += 1;
                }
            }
        }
    }

    for (&(x, y, z), block) in blocks {
        if (x >> 4) != chunk_x || (z >> 4) != chunk_z {
            continue;
        }
        dense[index(x, y, z)] = block.clone();
    }

    let mut heightmap = vec![0i32; 256];
    let mut sections = Vec::with_capacity(16);

    for section_y in 0..16usize {
        let slice = &dense[&section_y * 4096..(section_y + 1) * 4096];
        let mut block_ids = vec![0u8; 4096];
        let mut block_data = vec![0u8; 2048];
        let mut has_nonair = false;

        for (idx, block) in slice.iter().enumerate() {
            if block.is_air() {
                continue;
            }
            has_nonair = true;
            block_ids[idx] = block.id;

            let ni = idx / 2;
            if idx & 1 == 0 {
                block_data[ni] = (block_data[ni] & 0xF0) | (block.metadata & 0x0F);
            } else {
                block_data[ni] = (block_data[ni] & 0x0F) | ((block.metadata & 0x0F) << 4);
            }

            let column = idx & 0xFF;
            heightmap[column] = (section_y * 16 + (idx >> 8)) as i32 + 1;
        }

        // always keep section 0 so the chunk is never headerless
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

    let level = NbtTag::Compound(vec![
        ("xPos".to_string(), NbtTag::Int(chunk_x)),
        ("zPos".to_string(), NbtTag::Int(chunk_z)),
        ("LastUpdate".to_string(), NbtTag::Long(0)),
        ("LightPopulated".to_string(), NbtTag::Byte(1)),
        ("TerrainPopulated".to_string(), NbtTag::Byte(1)),
        ("V".to_string(), NbtTag::Byte(1)),
        ("InhabitedTime".to_string(), NbtTag::Long(0)),
        ("Biomes".to_string(), NbtTag::ByteArray(vec![1u8; 256])),
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
        TileEntity::Chest { items, .. } => {
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
        TileEntity::Furnace {
            input,
            fuel,
            output,
            burn_ticks,
            burn_ticks_total,
            cook_ticks,
            ..
        } => {
            let mut item_list = vec![];
            if let Some(s) = input {
                item_list.push(NbtTag::Compound(vec![
                    ("Slot".to_string(), NbtTag::Byte(0)),
                    ("id".to_string(), NbtTag::Int(s.item_id as i32)),
                    ("Count".to_string(), NbtTag::Byte(s.count as i8)),
                    ("Damage".to_string(), NbtTag::Short(s.metadata)),
                ]));
            }
            if let Some(s) = fuel {
                item_list.push(NbtTag::Compound(vec![
                    ("Slot".to_string(), NbtTag::Byte(1)),
                    ("id".to_string(), NbtTag::Int(s.item_id as i32)),
                    ("Count".to_string(), NbtTag::Byte(s.count as i8)),
                    ("Damage".to_string(), NbtTag::Short(s.metadata)),
                ]));
            }
            if let Some(s) = output {
                item_list.push(NbtTag::Compound(vec![
                    ("Slot".to_string(), NbtTag::Byte(2)),
                    ("id".to_string(), NbtTag::Int(s.item_id as i32)),
                    ("Count".to_string(), NbtTag::Byte(s.count as i8)),
                    ("Damage".to_string(), NbtTag::Short(s.metadata)),
                ]));
            }
            Some(NbtTag::Compound(vec![
                ("id".to_string(), NbtTag::String("Furnace".to_string())),
                ("x".to_string(), NbtTag::Int(x)),
                ("y".to_string(), NbtTag::Int(y)),
                ("z".to_string(), NbtTag::Int(z)),
                ("Items".to_string(), NbtTag::List(10, item_list)),
                ("BurnTime".to_string(), NbtTag::Short(*burn_ticks)),
                ("CookTime".to_string(), NbtTag::Short(*cook_ticks)),
                (
                    "CookTimeTotal".to_string(),
                    NbtTag::Short(*burn_ticks_total),
                ),
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

pub fn parse_tile_entity(raw: &RawTileEntity) -> Option<TileEntity> {
    match raw.id.as_str() {
        "Sign" => {
            let get_text = |key: &str| -> String {
                raw.compound
                    .get(key)
                    .and_then(|t| t.as_string())
                    .and_then(|json| {
                        json.strip_prefix(r#"{"text":""#)?
                            .strip_suffix(r#""}"#)
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default()
            };
            Some(TileEntity::Sign {
                lines: [
                    get_text("Text1"),
                    get_text("Text2"),
                    get_text("Text3"),
                    get_text("Text4"),
                ],
            })
        }
        "Chest" => {
            let mut items = vec![None; 27];
            if let Some(list) = raw.compound.get("Items").and_then(|t| t.as_list()) {
                for entry in list {
                    let slot = entry.get("Slot").and_then(|t| t.as_i8()).unwrap_or(0) as usize;
                    if slot < 27 {
                        items[slot] = Some(ItemStack {
                            item_id: entry.get("id").and_then(|t| t.as_i32()).unwrap_or(-1) as i16,
                            count: entry.get("Count").and_then(|t| t.as_i8()).unwrap_or(0) as u8,
                            metadata: entry
                                .get("Damage")
                                .and_then(|t| t.as_i16_val())
                                .unwrap_or(0),
                            durability: 0,
                        });
                    }
                }
            }
            Some(TileEntity::Chest { items, viewers: 0 })
        }
        "Furnace" => {
            let mut input = None;
            let mut fuel = None;
            let mut output = None;

            if let Some(list) = raw.compound.get("Items").and_then(|t| t.as_list()) {
                for entry in list {
                    let slot = entry.get("Slot").and_then(|t| t.as_i8()).unwrap_or(-1);
                    let stack = ItemStack {
                        item_id: entry.get("id").and_then(|t| t.as_i32()).unwrap_or(-1) as i16,
                        count: entry.get("Count").and_then(|t| t.as_i8()).unwrap_or(0) as u8,
                        metadata: entry
                            .get("Damage")
                            .and_then(|t| t.as_i16_val())
                            .unwrap_or(0),
                        durability: 0,
                    };
                    match slot {
                        0 => input = Some(stack),
                        1 => fuel = Some(stack),
                        2 => output = Some(stack),
                        _ => {}
                    }
                }
            }

            let burn_ticks = raw
                .compound
                .get("BurnTime")
                .and_then(|t| t.as_i16_val())
                .unwrap_or(0);
            let cook_ticks = raw
                .compound
                .get("CookTime")
                .and_then(|t| t.as_i16_val())
                .unwrap_or(0);
            let burn_ticks_total = raw
                .compound
                .get("CookTimeTotal")
                .and_then(|t| t.as_i16_val())
                .unwrap_or(0);

            Some(TileEntity::Furnace {
                input,
                fuel,
                output,
                burn_ticks,
                burn_ticks_total,
                cook_ticks,
                viewers: 0,
            })
        }
        _ => None,
    }
}

pub fn nbt_to_blocks(
    nbt_data: &[u8],
    out: &mut HashMap<(i32, u8, i32), Block>,
    generator: &dyn WorldGenerator,
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

            for z in 0..16usize {
                for x in 0..16usize {
                    let idx = local_y * 256 + z * 16 + x;
                    let wx = chunk_x * 16 + x as i32;
                    let wz = chunk_z * 16 + z as i32;
                    let gen_block = generator.block_at(wx, wy, wz);
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

                    // skip if matches generator - no need to store
                    if block_id == gen_block.id && metadata == gen_block.metadata {
                        continue;
                    }

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

    out.reserve(sections.len() * 4096);
    let (base_x, base_z) = (chunk_x * 16, chunk_z * 16);

    for section in sections {
        let Some(section_y) = section.get("Y").and_then(|v| v.as_i8()) else {
            continue;
        };
        let Some(block_ids) = section.get("Blocks").and_then(|v| v.as_byte_array()) else {
            continue;
        };

        if block_ids.len() < 4096 {
            continue;
        }
        let block_data = section
            .get("Data")
            .and_then(|v| v.as_byte_array())
            .filter(|d| d.len() >= 2048);
        let y_start = (section_y as i32) * 16;

        for (idx, &block_id) in block_ids[..4096].iter().enumerate() {
            if block_id == 0 {
                continue;
            }
            let metadata = match block_data {
                Some(d) => {
                    let packed = d[idx / 2];
                    if idx & 1 == 0 {
                        packed & 0x0F
                    } else {
                        (packed >> 4) & 0x0F
                    }
                }
                None => 0,
            };
            let wx = base_x + (idx & 15) as i32;
            let wz = base_z + ((idx >> 4) & 15) as i32;
            let wy = (y_start + (idx >> 8) as i32) as u8;
            out.insert((wx, wy, wz), Block::new(block_id, metadata));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{FlatWorldGenerator, VoidWorldGenerator};

    fn level(bytes: &[u8]) -> NbtTag {
        let (_, root) = NbtReader::new(bytes).read_named_root();
        root.get("Level").expect("Level").clone()
    }

    fn heightmap(bytes: &[u8]) -> Vec<i32> {
        match level(bytes).get("HeightMap").expect("HeightMap") {
            NbtTag::IntArray(v) => v.clone(),
            other => panic!("{other:?}"),
        }
    }

    fn sections(bytes: &[u8]) -> usize {
        level(bytes)
            .get("Sections")
            .and_then(|s| s.as_list())
            .map(|s| s.len())
            .unwrap_or(0)
    }

    fn solid_chunk() -> HashMap<(i32, u8, i32), Block> {
        let mut m = HashMap::new();
        for x in 0..16i32 {
            for z in 0..16i32 {
                for y in 0..64u8 {
                    m.insert((x, y, z), Block::new(if y == 63 { 2 } else { 1 }, 0));
                }
            }
        }
        m
    }

    #[test]
    fn blocks_round_trip_through_the_encoder() {
        let g = VoidWorldGenerator::new();
        let mut blocks = HashMap::new();
        for i in 0..16u8 {
            blocks.insert((i as i32, i * 16, 3), Block::new(35, i));
        }
        blocks.insert((15, 255, 15), Block::new(7, 0));

        let nbt = chunk_to_nbt(0, 0, &blocks, &g, Vec::new(), false);
        let mut back = HashMap::new();
        nbt_to_blocks_raw(&nbt, &mut back);

        assert_eq!(back.len(), blocks.len());
        for (pos, block) in &blocks {
            let got = back.get(pos).unwrap_or_else(|| panic!("{pos:?} was lost"));
            assert_eq!(
                (got.id, got.metadata),
                (block.id, block.metadata),
                "at {pos:?}"
            );
        }
    }

    #[test]
    fn a_generated_chunk_round_trips_its_terrain() {
        let g = FlatWorldGenerator::new();
        let nbt = chunk_to_nbt(2, -3, &HashMap::new(), &g, Vec::new(), true);
        let mut back = HashMap::new();
        nbt_to_blocks_raw(&nbt, &mut back);

        // 4 layers over a 16x16 chunk, and nothing above
        assert_eq!(back.len(), 16 * 16 * 4);
        assert_eq!(back.get(&(32, 0, -48)).map(|b| b.id), Some(7));
        assert_eq!(back.get(&(32, 3, -48)).map(|b| b.id), Some(2));
        assert!(back.get(&(32, 4, -48)).is_none());
    }

    #[test]
    fn the_heightmap_describes_the_blocks_not_the_generator() {
        let flat = FlatWorldGenerator::new();

        let empty = chunk_to_nbt(0, 0, &HashMap::new(), &flat, Vec::new(), false);
        assert!(heightmap(&empty).iter().all(|&h| h == 0));

        let mut blocks = HashMap::new();
        blocks.insert((0, 0, 0), Block::new(7, 0));
        let dug = chunk_to_nbt(0, 0, &blocks, &flat, Vec::new(), false);
        let h = heightmap(&dug);
        assert_eq!(h[0], 1, "one block at y=0 is a height of 1");
        assert_eq!(h[1], 0, "the column beside it is empty");

        let generated = chunk_to_nbt(0, 0, &HashMap::new(), &flat, Vec::new(), true);
        assert!(heightmap(&generated).iter().all(|&h| h == 4));
    }

    #[test]
    fn a_block_on_the_top_layer_counts() {
        let g = VoidWorldGenerator::new();
        let mut blocks = HashMap::new();
        blocks.insert((0, 255, 0), Block::new(1, 0));
        assert_eq!(
            heightmap(&chunk_to_nbt(0, 0, &blocks, &g, Vec::new(), false))[0],
            256
        );
    }

    #[test]
    fn empty_sections_are_dropped_but_section_zero_is_kept() {
        let g = VoidWorldGenerator::new();
        assert_eq!(
            sections(&chunk_to_nbt(0, 0, &HashMap::new(), &g, Vec::new(), false)),
            1
        );

        let mut blocks = HashMap::new();
        blocks.insert((0, 200, 0), Block::new(1, 0));
        assert_eq!(
            sections(&chunk_to_nbt(0, 0, &blocks, &g, Vec::new(), false)),
            2
        );
    }

    #[test]
    fn blocks_from_another_chunk_are_ignored() {
        let g = VoidWorldGenerator::new();
        let mut blocks = HashMap::new();
        blocks.insert((3, 10, 3), Block::new(1, 0));
        blocks.insert((19, 10, 3), Block::new(49, 0)); // chunk (1,0)
        blocks.insert((3, 10, -13), Block::new(49, 0)); // chunk (0,-1)

        let nbt = chunk_to_nbt(0, 0, &blocks, &g, Vec::new(), false);
        let mut back = HashMap::new();
        nbt_to_blocks_raw(&nbt, &mut back);
        assert_eq!(back.len(), 1);
        assert_eq!(back.get(&(3, 10, 3)).map(|b| b.id), Some(1));
    }

    #[test]
    fn negative_chunk_coordinates_keep_their_positions() {
        let g = VoidWorldGenerator::new();
        let mut blocks = HashMap::new();
        blocks.insert((-48, 7, -17), Block::new(1, 0));

        let nbt = chunk_to_nbt(-3, -2, &blocks, &g, Vec::new(), false);
        let mut back = HashMap::new();
        nbt_to_blocks_raw(&nbt, &mut back);
        assert_eq!(back.keys().collect::<Vec<_>>(), vec![&(-48, 7, -17)]);
    }

    #[test]
    fn a_solid_chunk_survives_a_full_cycle() {
        let g = FlatWorldGenerator::new();
        let blocks = solid_chunk();
        let nbt = chunk_to_nbt(0, 0, &blocks, &g, Vec::new(), false);

        let mut back = HashMap::new();
        nbt_to_blocks_raw(&nbt, &mut back);
        assert_eq!(back.len(), blocks.len());

        assert_eq!(chunk_to_nbt(0, 0, &back, &g, Vec::new(), false), nbt);
        assert_eq!(heightmap(&nbt)[0], 64);
    }
}
