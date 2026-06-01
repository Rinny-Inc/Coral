use crate::protocol::{packets::Packet, writer::Writer};
use std::io::Write;
use std::io::{Error, Result};

const BEDROCK: u8 = 7;
const DIRT: u8 = 3;
const GRASS: u8 = 2;
const AIR: u8 = 0;

fn index_yzx(x: usize, y: usize, z: usize) -> usize {
    (y * 256) + (z * 16) + x
}

fn build_flat_section() -> Vec<u8> {
    let mut blocks = vec![AIR; 16 * 16 * 16];

    for x in 0..16usize {
        for z in 0..16usize {
            for y in 0..16usize {
                let block = match y {
                    0 => BEDROCK,
                    1 | 2 => DIRT,
                    3 => GRASS,
                    _ => AIR,
                };
                blocks[index_yzx(x, y, z)] = block;
            }
        }
    }

    blocks
}

fn build_chunk_data_18() -> Vec<u8> {
    let mut data = Vec::new();

    // 2 bytes per block (u16)
    // Formula => (Id << 4) | Metadata
    for y in 0..16usize {
        for _z in 0..16usize {
            for _x in 0..16usize {
                let block_id = match y {
                    0 => BEDROCK as u16,
                    1 | 2 => DIRT as u16,
                    3 => GRASS as u16,
                    _ => AIR as u16,
                };
                let metadata = 0u16; // 0u16 metadata
                let block_state = (block_id << 4) | (metadata & 0xF);

                // Little-Endian u16 (2 bytes)
                data.extend_from_slice(&block_state.to_le_bytes());
            }
        }
    }

    // 2. Block light (4 bits per block = 2048 bytes)
    data.extend_from_slice(&vec![0xFFu8; 2048]);

    // 3. Sky light (4 bits per block = 2048 bytes)
    data.extend_from_slice(&vec![0xFFu8; 2048]);

    // 4. Biomes (256 bytes) - Only appended if ground_up_continuous = true
    data.extend_from_slice(&vec![1u8; 256]);

    data
}

// FIXME: packet length too large by 1 byte
fn build_chunk_data_17() -> Vec<u8> {
    let mut data = Vec::new();
    let blocks = build_flat_section(); // Updated to use the correct YZX index

    // 1. Block IDs (4096 bytes)
    data.extend_from_slice(&blocks);

    // 2. Block Metadata (2048 bytes)
    data.extend_from_slice(&vec![0u8; 2048]);

    // 3. Block Light (2048 bytes)
    data.extend_from_slice(&vec![0xFFu8; 2048]);

    // 4. Sky Light (2048 bytes)
    data.extend_from_slice(&vec![0xFFu8; 2048]);

    // 5. Biomes (256 bytes) - Only if ground_up_continuous is true
    data.extend_from_slice(&vec![1u8; 256]);

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
pub struct ChunkData {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub client_protocol: i32,
}

impl Packet for ChunkData {
    fn decode(_buf: &mut bytes::Bytes) -> Result<Self> {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut Writer) -> Result<()> {
        writer.write_varint(0x21);
        writer.write_i32(self.chunk_x);
        writer.write_i32(self.chunk_z);
        writer.write_bool(true); // ground-up continuous

        if self.client_protocol == 47 {
            // 1.8 - primary bit mask: bit 0 set = section 0 present
            writer.write_u16(0x0001);

            let data = build_chunk_data_18();
            writer.write_varint(data.len() as i32);
            writer.data.extend_from_slice(&data);
        } else {
            // 1.7 - primary bit mask + add bit mask
            writer.write_u16(0x0001); // section 0 present
            writer.write_u16(0x0000); // no add data

            let data = build_chunk_data_17();
            let compressed = zlib_compress(&data);
            writer.write_i32(compressed.len() as i32);
            writer.data.extend_from_slice(&compressed);
        }

        println!(
            "DEBUG ChunkData ({},{}) protocol={} data_len={}",
            self.chunk_x,
            self.chunk_z,
            self.client_protocol,
            // log the size before writing
            if self.client_protocol == 47 {
                build_chunk_data_18().len()
            } else {
                build_chunk_data_17().len()
            }
        );

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
