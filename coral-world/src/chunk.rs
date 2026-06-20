use crate::anvil::nbt_to_blocks_raw;
use crate::blocks::Block;
use crate::blocks::WorldBlocks;
use crate::generator::FlatWorldGenerator;
use coral_protocol::packets::PacketOut;
use coral_protocol::writer::Writer;
use std::collections::HashMap;
use std::io::Result;
use std::io::Write;
use std::sync::Arc;

#[derive(Debug)]
pub struct ChunkData {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub client_protocol: i32,
    pub data: Vec<u8>,
    pub primary_bitmask: u16,
}
impl ChunkData {
    pub async fn build(
        chunk_x: i32,
        chunk_z: i32,
        client_protocol: i32,
        world_blocks: &Arc<WorldBlocks>,
        generator: &FlatWorldGenerator,
    ) -> Self {
        if let Some(nbt) = world_blocks.get_chunk_nbt(chunk_x, chunk_z).await {
            let mut chunk_blocks: HashMap<(i32, u8, i32), Block> = HashMap::new();
            nbt_to_blocks_raw(&nbt, &mut chunk_blocks);

            let mut found_sections: Vec<i32> = chunk_blocks
                .keys()
                .map(|(_, y, _)| (*y as i32) / 16)
                .collect();
            found_sections.sort();
            found_sections.dedup();

            let player_blocks = world_blocks.blocks.read().await;
            for x in 0..16i32 {
                for z in 0..16i32 {
                    for y in 0u8..=255 {
                        let wx = chunk_x * 16 + x;
                        let wz = chunk_z * 16 + z;
                        if let Some(b) = player_blocks.get(&(wx, y, wz)) {
                            chunk_blocks.insert((wx, y, wz), b.clone());
                        }
                    }
                }
            }
            let (data, primary_bitmask) =
                build_chunk_data_18_from_map(chunk_x, chunk_z, &chunk_blocks);

            //println!("DEBUG ChunkData ({},{}) data_len={}", chunk_x, chunk_z, data.len());

            return Self {
                chunk_x,
                chunk_z,
                client_protocol,
                data,
                primary_bitmask,
            };
        }
        let data = if client_protocol == 47 {
            build_chunk_data_18(chunk_x, chunk_z, world_blocks, generator).await
        } else {
            build_chunk_data_17(chunk_x, chunk_z, world_blocks, generator).await
        };

        //println!("DEBUG ChunkData ({},{}) data_len={}", chunk_x, chunk_z, data.len());

        Self {
            chunk_x,
            chunk_z,
            client_protocol,
            data,
            primary_bitmask: 0x0001,
        }
    }
}

impl PacketOut for ChunkData {
    fn encode(&self, writer: &mut Writer) -> Result<()> {
        writer.write_varint(0x21);
        writer.write_i32(self.chunk_x);
        writer.write_i32(self.chunk_z);
        writer.write_bool(true); // ground-up continuous

        if self.client_protocol == 47 {
            // 1.8 - primary bit mask: bit 0 set = section 0 present
            writer.write_u16(self.primary_bitmask);
            writer.write_varint(self.data.len() as i32);
            writer.data.extend_from_slice(&self.data);
        } else {
            // 1.7 - primary bit mask + add bit mask
            writer.write_u16(0x0001); // section 0 present
            writer.write_u16(0x0000); // no add data

            let compressed = zlib_compress(&self.data);
            writer.write_i32(compressed.len() as i32);
            writer.data.extend_from_slice(&compressed);
        }

        Ok(())
    }
}

fn get_block_state(
    snapshot: &std::collections::HashMap<(i32, u8, i32), Block>,
    wx: i32,
    wy: u8,
    wz: i32,
    generator: &FlatWorldGenerator,
) -> u16 {
    let block = snapshot
        .get(&(wx, wy, wz))
        .cloned()
        .unwrap_or_else(|| generator.get(wy));
    ((block.id as u16) << 4) | (block.metadata as u16 & 0xF)
}

async fn build_chunk_data_18(
    chunk_x: i32,
    chunk_z: i32,
    world_blocks: &Arc<WorldBlocks>,
    generator: &FlatWorldGenerator,
) -> Vec<u8> {
    let block_data = {
        let snapshot = world_blocks.blocks.read().await;
        let mut data = Vec::with_capacity(8192);
        for y in 0..16usize {
            for z in 0..16usize {
                for x in 0..16usize {
                    let wx = chunk_x * 16 + x as i32;
                    let wz = chunk_z * 16 + z as i32;
                    let state = get_block_state(&snapshot, wx, y as u8, wz, generator);
                    data.extend_from_slice(&state.to_le_bytes());
                }
            }
        }
        data
    };

    let mut data = Vec::with_capacity(12544);
    data.extend_from_slice(&block_data);
    data.extend_from_slice(&vec![0xFFu8; 2048]); // block light
    data.extend_from_slice(&vec![0xFFu8; 2048]); // sky light
    data.extend_from_slice(&vec![1u8; 256]); // biomes
    data
}

// FIXME: packet length too large by 1 byte
// is it fixed???
async fn build_chunk_data_17(
    chunk_x: i32,
    chunk_z: i32,
    world_blocks: &Arc<WorldBlocks>,
    generator: &FlatWorldGenerator,
) -> Vec<u8> {
    let mut blocks = vec![0u8; 4096];

    for y in 0..16usize {
        for z in 0..16usize {
            for x in 0..16usize {
                let wx = chunk_x * 16 + x as i32;
                let wy = y as u8;
                let wz = chunk_z * 16 + z as i32;

                let block = world_blocks.get(wx, wy, wz, generator).await;
                let index = y * 256 + z * 16 + x;
                blocks[index] = block.id;
            }
        }
    }

    let mut data = Vec::with_capacity(10496);
    data.extend_from_slice(&blocks);
    data.extend_from_slice(&vec![0u8; 2048]); // metadata
    data.extend_from_slice(&vec![0xFFu8; 2048]); // block light
    data.extend_from_slice(&vec![0xFFu8; 2048]); // sky light
    data.extend_from_slice(&vec![1u8; 256]); // biomes
    data
}

pub fn build_chunk_data_18_from_map(
    chunk_x: i32,
    chunk_z: i32,
    chunk_blocks: &HashMap<(i32, u8, i32), Block>,
) -> (Vec<u8>, u16) {
    let mut bitmask: u16 = 0;

    let mut section_has_blocks = [false; 16];
    for section_y in 0..16u8 {
        let y_start = section_y as i32 * 16;
        let mut has_blocks = false;
        'check: for y in 0..16i32 {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    let wx = chunk_x * 16 + x;
                    let wz = chunk_z * 16 + z;
                    let wy = (y_start + y) as u8;
                    if chunk_blocks
                        .get(&(wx, wy, wz))
                        .map(|b| b.id != 0)
                        .unwrap_or(false)
                    {
                        has_blocks = true;
                        break 'check;
                    }
                }
            }
        }
        section_has_blocks[section_y as usize] = has_blocks || section_y == 0;
        if section_has_blocks[section_y as usize] {
            bitmask |= 1 << section_y;
        }
    }

    let mut block_arrays = Vec::new();
    for section_y in 0..16u8 {
        if !section_has_blocks[section_y as usize] {
            continue;
        }
        let y_start = section_y as i32 * 16;
        let mut block_section = Vec::with_capacity(4096 * 2);
        for y in 0..16i32 {
            for z in 0..16i32 {
                for x in 0..16i32 {
                    let wx = chunk_x * 16 + x;
                    let wz = chunk_z * 16 + z;
                    let wy = (y_start + y) as u8;
                    let block = chunk_blocks
                        .get(&(wx, wy, wz))
                        .cloned()
                        .unwrap_or_else(Block::air);
                    let state = ((block.id as u16) << 4) | (block.metadata as u16 & 0xF);
                    block_section.extend_from_slice(&state.to_le_bytes());
                }
            }
        }
        block_arrays.push(block_section);
    }

    let mut block_light_count = 0;
    for section_y in 0..16u8 {
        if section_has_blocks[section_y as usize] {
            block_light_count += 1;
        }
    }

    let mut data = Vec::new();
    for arr in &block_arrays {
        data.extend_from_slice(arr);
    }
    for _ in 0..block_light_count {
        data.extend_from_slice(&[0xFFu8; 2048]); // block light
    }
    for _ in 0..block_light_count {
        data.extend_from_slice(&[0xFFu8; 2048]); // sky light
    }
    data.extend_from_slice(&[1u8; 256]); // biomes

    (data, bitmask)
}

pub fn build_chunk_data_17_from_map(
    chunk_x: i32,
    chunk_z: i32,
    chunk_blocks: &HashMap<(i32, u8, i32), Block>,
) -> Vec<u8> {
    let mut blocks = vec![0u8; 4096];
    let mut metadata = vec![0u8; 2048];

    for y in 0..16usize {
        for z in 0..16usize {
            for x in 0..16usize {
                let wx = chunk_x * 16 + x as i32;
                let wz = chunk_z * 16 + z as i32;
                let wy = y as u8;
                let block = chunk_blocks
                    .get(&(wx, wy, wz))
                    .cloned()
                    .unwrap_or_else(Block::air);
                let index = y * 256 + z * 16 + x;
                blocks[index] = block.id;

                // nibble metadata
                let ni = index / 2;
                if index & 1 == 0 {
                    metadata[ni] = (metadata[ni] & 0xF0) | (block.metadata & 0x0F);
                } else {
                    metadata[ni] = (metadata[ni] & 0x0F) | ((block.metadata & 0x0F) << 4);
                }
            }
        }
    }

    let mut data = Vec::with_capacity(10496);
    data.extend_from_slice(&blocks);
    data.extend_from_slice(&metadata);
    data.extend_from_slice(&vec![0xFFu8; 2048]); // block light
    data.extend_from_slice(&vec![0xFFu8; 2048]); // sky light
    data.extend_from_slice(&vec![1u8; 256]); // biomes
    data
}

fn zlib_compress(data: &[u8]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::DeflateEncoder;

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[derive(Debug)]
pub struct UnloadChunk {
    pub chunk_x: i32,
    pub chunk_z: i32,
}
impl PacketOut for UnloadChunk {
    fn encode(&self, writer: &mut Writer) -> std::io::Result<()> {
        writer.write_varint(0x21);
        writer.write_i32(self.chunk_x);
        writer.write_i32(self.chunk_z);
        writer.write_bool(true);
        writer.write_u16(0x0000);
        writer.write_varint(0);
        Ok(())
    }
}
