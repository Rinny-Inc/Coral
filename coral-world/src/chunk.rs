use crate::anvil::nbt_to_blocks_raw;
use crate::blocks::Block;
use crate::blocks::WorldBlocks;
use crate::generator::WorldGenerator;
use crate::world::World;
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
        world: &Arc<World>,
    ) -> Self {
        if let Some(nbt) = world.blocks.get_chunk_nbt(chunk_x, chunk_z).await {
            let mut chunk_blocks: HashMap<(i32, u8, i32), Block> = HashMap::new();
            nbt_to_blocks_raw(&nbt, &mut chunk_blocks);

            for (pos, block) in world.blocks.chunk_edits(chunk_x, chunk_z).await {
                chunk_blocks.insert(pos, block);
            }
            let (data, primary_bitmask) = if client_protocol == 47 {
                build_chunk_data_18_from_map(chunk_x, chunk_z, &chunk_blocks)
            } else {
                todo!()
            };

            //tracing::debug!("DEBUG ChunkData ({},{}) data_len={}", chunk_x, chunk_z, data.len());

            return Self {
                chunk_x,
                chunk_z,
                client_protocol,
                data,
                primary_bitmask,
            };
        }

        if !world.generator.generates_in_chunk(chunk_x, chunk_z)
            && !world.blocks.chunk_has_edits(chunk_x, chunk_z).await
        {
            return Self {
                chunk_x,
                chunk_z,
                client_protocol,
                data: vec![1u8; 256],
                primary_bitmask: 0x0000,
            };
        }

        let data = if client_protocol == 47 {
            build_chunk_data_18(chunk_x, chunk_z, &world.blocks, world.generator.as_ref()).await
        } else {
            build_chunk_data_17(chunk_x, chunk_z, &world.blocks, world.generator.as_ref()).await
        };

        //tracing::debug!("DEBUG ChunkData ({},{}) data_len={}", chunk_x, chunk_z, data.len());

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
    snapshot: &crate::blocks::ChunkEdits,
    wx: i32,
    wy: u8,
    wz: i32,
    generator: &dyn WorldGenerator,
) -> u16 {
    let block = snapshot
        .get(&(wx, wy, wz))
        .cloned()
        .unwrap_or_else(|| generator.block_at(wx, wy, wz));
    ((block.id as u16) << 4) | (block.metadata as u16 & 0xF)
}

async fn build_chunk_data_18(
    chunk_x: i32,
    chunk_z: i32,
    world_blocks: &WorldBlocks,
    generator: &dyn WorldGenerator,
) -> Vec<u8> {
    let snapshot = world_blocks.chunk_edits(chunk_x, chunk_z).await;
    let block_data = {
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
    world_blocks: &WorldBlocks,
    generator: &dyn WorldGenerator,
) -> Vec<u8> {
    let mut blocks = vec![0u8; 4096];
    let snapshot = world_blocks.chunk_edits(chunk_x, chunk_z).await;

    for y in 0..16usize {
        for z in 0..16usize {
            for x in 0..16usize {
                let wx = chunk_x * 16 + x as i32;
                let wy = y as u8;
                let wz = chunk_z * 16 + z as i32;

                let block = snapshot
                    .get(&(wx, wy, wz))
                    .cloned()
                    .unwrap_or_else(|| generator.block_at(wx, wy, wz));
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
    const VOLUME: usize = 16 * 16 * 256;

    let mut dense = vec![0u16; VOLUME];
    let mut section_has_blocks = [false; 16];
    for (&(x, y, z), block) in chunk_blocks {
        if block.is_air() || (x >> 4) != chunk_x || (z >> 4) != chunk_z {
            continue;
        }
        let idx = ((y as usize) << 8) | (((z & 15) as usize) << 4) | ((x & 15) as usize);
        dense[idx] = ((block.id as u16) << 4) | (block.metadata as u16 & 0xF);
        section_has_blocks[(y >> 4) as usize] = true;
    }
    section_has_blocks[0] = true;

    let mut bitmask = 0u16;
    let mut section_count = 0usize;
    for (section_y, present) in section_has_blocks.iter().enumerate() {
        if *present {
            bitmask |= 1 << section_y;
            section_count += 1;
        }
    }

    let mut data = Vec::with_capacity(section_count * (8192 + 4096) + 256);
    for (section_y, present) in section_has_blocks.iter().enumerate() {
        if !*present {
            continue;
        }
        for state in &dense[section_y * 4096..(section_y + 1) * 4096] {
            data.extend_from_slice(&state.to_le_bytes());
        }
    }
    for _ in 0..section_count {
        data.extend_from_slice(&[0xFFu8; 2048]); // block light
    }
    for _ in 0..section_count {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::VoidWorldGenerator;
    use crate::world::World;

    fn void() -> Arc<World> {
        Arc::new(World::new(Arc::new(VoidWorldGenerator::new())))
    }

    #[tokio::test]
    async fn a_void_chunk_is_sent_as_no_sections() {
        let c = ChunkData::build(0, 0, 47, &void()).await;
        assert_eq!(c.primary_bitmask, 0x0000);
        assert_eq!(c.data.len(), 256, "biomes only");
    }

    #[tokio::test]
    async fn an_edited_void_chunk_carries_its_section() {
        let w = void();
        w.set(3, 5, 3, Block::new(1, 0)).await;

        let c = ChunkData::build(0, 0, 47, &w).await;
        assert_eq!(c.primary_bitmask, 0x0001);
        assert_eq!(c.data.len(), 12544);

        let idx = ((5usize & 15) << 8 | 3 << 4 | 3) * 2;
        let state = u16::from_le_bytes([c.data[idx], c.data[idx + 1]]);
        assert_eq!(state >> 4, 1, "stone id at the placed position");

        let n = ChunkData::build(1, 0, 47, &w).await;
        assert_eq!(n.primary_bitmask, 0x0000);
    }

    #[tokio::test]
    async fn a_flat_chunk_still_claims_its_section() {
        let w = Arc::new(World::flat());
        let c = ChunkData::build(0, 0, 47, &w).await;
        assert_eq!(c.primary_bitmask, 0x0001);
        assert_eq!(c.data.len(), 12544);

        // bedrock at y=0, grass at y=3, air above
        let at = |x: usize, y: usize, z: usize| {
            let i = (y << 8 | z << 4 | x) * 2;
            u16::from_le_bytes([c.data[i], c.data[i + 1]]) >> 4
        };
        assert_eq!(at(0, 0, 0), 7);
        assert_eq!(at(0, 3, 0), 2);
        assert_eq!(at(0, 4, 0), 0);
    }
}

#[cfg(test)]
mod from_map_tests {
    use super::*;

    fn states(data: &[u8], section: usize) -> Vec<u16> {
        data[section * 8192..(section + 1) * 8192]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    fn at(data: &[u8], section: usize, x: usize, y: usize, z: usize) -> (u8, u8) {
        let s = states(data, section)[(y << 8) | (z << 4) | x];
        ((s >> 4) as u8, (s & 0xF) as u8)
    }

    #[test]
    fn an_empty_chunk_still_ships_section_zero() {
        let (data, bitmask) = build_chunk_data_18_from_map(0, 0, &HashMap::new());
        assert_eq!(bitmask, 0x0001);
        assert_eq!(data.len(), 8192 + 2048 + 2048 + 256);
    }

    #[test]
    fn the_bitmask_names_exactly_the_populated_sections() {
        let mut blocks = HashMap::new();
        blocks.insert((0, 20, 0), Block::new(1, 0)); // section 1
        blocks.insert((0, 200, 0), Block::new(1, 0)); // section 12
        let (data, bitmask) = build_chunk_data_18_from_map(0, 0, &blocks);

        // 0 is always sent, plus 1 & 12
        assert_eq!(bitmask, (1 << 0) | (1 << 1) | (1 << 12));
        assert_eq!(data.len(), 3 * (8192 + 2048 + 2048) + 256);
    }

    #[test]
    fn blocks_land_at_the_yzx_index_the_client_reads() {
        let mut blocks = HashMap::new();
        blocks.insert((3, 5, 7), Block::new(1, 0));
        blocks.insert((15, 0, 15), Block::new(7, 0));
        let (data, _) = build_chunk_data_18_from_map(0, 0, &blocks);

        assert_eq!(at(&data, 0, 3, 5, 7), (1, 0));
        assert_eq!(at(&data, 0, 15, 0, 15), (7, 0));
        assert_eq!(at(&data, 0, 0, 0, 0), (0, 0), "untouched positions are air");
    }

    #[test]
    fn every_metadata_value_survives() {
        let mut blocks = HashMap::new();
        for i in 0..16u8 {
            blocks.insert((i as i32, 1, 0), Block::new(35, i));
        }
        let (data, _) = build_chunk_data_18_from_map(0, 0, &blocks);
        for i in 0..16usize {
            assert_eq!(at(&data, 0, i, 1, 0), (35, i as u8), "metadata {i}");
        }
    }

    #[test]
    fn negative_chunk_coordinates_use_the_low_nibble() {
        let mut blocks = HashMap::new();
        blocks.insert((-48, 4, 112), Block::new(1, 0));
        blocks.insert((-33, 4, 127), Block::new(2, 0));
        let (data, _) = build_chunk_data_18_from_map(-3, 7, &blocks);
        assert_eq!(at(&data, 0, 0, 4, 0), (1, 0));
        assert_eq!(at(&data, 0, 15, 4, 15), (2, 0));
    }

    #[test]
    fn blocks_from_another_chunk_are_ignored() {
        let mut blocks = HashMap::new();
        blocks.insert((3, 10, 3), Block::new(1, 0));
        blocks.insert((19, 10, 3), Block::new(49, 0)); // chunk (1,0)
        let (data, _) = build_chunk_data_18_from_map(0, 0, &blocks);
        assert_eq!(at(&data, 0, 3, 10, 3), (1, 0));
    }

    #[test]
    fn explicit_air_does_not_create_a_section() {
        let mut blocks = HashMap::new();
        blocks.insert((1, 200, 1), Block::new(0, 0));
        let (_, bitmask) = build_chunk_data_18_from_map(0, 0, &blocks);
        assert_eq!(bitmask, 0x0001, "air created section 12");
    }

    #[test]
    fn a_block_on_the_top_layer_is_in_the_last_section() {
        let mut blocks = HashMap::new();
        blocks.insert((0, 255, 0), Block::new(1, 0));
        let (data, bitmask) = build_chunk_data_18_from_map(0, 0, &blocks);
        assert_eq!(bitmask, 0x0001 | (1 << 15));
        assert_eq!(at(&data, 1, 0, 15, 0), (1, 0));
    }
}

#[cfg(test)]
mod spawn_platform_tests {
    use std::sync::Arc;

    use super::ChunkData;
    use crate::{generator::VoidWorldGenerator, world::World};

    const STONE: u8 = 1;

    fn void_at_spawn(x: i32, y: f64, z: i32) -> Arc<World> {
        Arc::new(World::new(Arc::new(
            VoidWorldGenerator::with_spawn_platform(x, y, z),
        )))
    }

    #[tokio::test]
    async fn the_world_reads_the_platform_back() {
        let world = void_at_spawn(0, 65.0, 0);
        assert_eq!(world.get(0, 64, 0).await.id, STONE);
        assert!(world.get(0, 65, 0).await.is_air());
        assert!(world.get(1, 64, 0).await.is_air());
    }

    #[tokio::test]
    async fn the_spawn_is_not_floating_above_the_platform() {
        let world = void_at_spawn(0, 65.0, 0);
        assert_eq!(world.surface_y(0, 0).await, Some(65));
        assert!(
            !world.is_floating_spawn(0.5, 65.0, 0.5).await,
            "the player would still be told they are about to fall"
        );
        assert_eq!(world.surface_y(1, 0).await, None);
    }

    #[tokio::test]
    async fn the_spawn_chunk_is_not_sent_as_empty() {
        let world = void_at_spawn(0, 65.0, 0);
        let chunk = ChunkData::build(0, 0, 47, &world).await;
        assert_ne!(
            chunk.primary_bitmask, 0x0000,
            "the spawn chunk went out empty"
        );
    }

    #[tokio::test]
    async fn every_other_chunk_is_still_sent_empty() {
        let world = void_at_spawn(0, 65.0, 0);
        for (cx, cz) in [(1, 0), (0, 1), (-1, -1), (40, -40)] {
            let chunk = ChunkData::build(cx, cz, 47, &world).await;
            assert_eq!(
                chunk.primary_bitmask, 0x0000,
                "chunk ({cx}, {cz}) was built when it had nothing in it"
            );
        }
    }

    #[tokio::test]
    async fn a_platform_away_from_the_origin_lands_in_its_own_chunk() {
        // (-40, 137) is chunk (-3, 8)
        let world = void_at_spawn(-40, 65.0, 137);
        assert_ne!(ChunkData::build(-3, 8, 47, &world).await.primary_bitmask, 0);
        assert_eq!(ChunkData::build(0, 0, 47, &world).await.primary_bitmask, 0);
        assert_eq!(world.get(-40, 64, 137).await.id, STONE);
    }
}
