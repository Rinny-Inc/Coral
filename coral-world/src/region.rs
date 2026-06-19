use std::{
    collections::HashSet,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};

pub struct RegionFile {
    pub path: PathBuf,
}
impl RegionFile {
    pub fn new(world_dir: &Path, region_x: i32, region_z: i32) -> Self {
        let path = world_dir
            .join("region")
            .join(format!("r.{}.{}.mca", region_x, region_z));
        Self { path }
    }

    pub fn read_chunk(&self, chunk_x: i32, chunk_z: i32) -> Option<Vec<u8>> {
        let data = std::fs::read(&self.path).ok()?;
        if data.len() < 8192 {
            return None;
        }

        let local_x = chunk_x.rem_euclid(32);
        let local_z = chunk_z.rem_euclid(32);
        let header_offset = (local_x + local_z * 32) as usize * 4;

        let offset_bytes = &data[header_offset..header_offset + 4];
        let offset = ((offset_bytes[0] as u32) << 16
            | (offset_bytes[1] as u32) << 8
            | offset_bytes[2] as u32) as usize;
        let _sector_count = offset_bytes[3] as usize;

        if offset < 2 {
            return None; // chunk not present
        }

        let byte_offset = offset * 4096;
        if byte_offset + 5 > data.len() {
            return None;
        }

        let length =
            u32::from_be_bytes(data[byte_offset..byte_offset + 4].try_into().ok()?) as usize;
        let compression = data[byte_offset + 4];

        if byte_offset + 5 + length - 1 > data.len() {
            return None;
        }

        let compressed = &data[byte_offset + 5..byte_offset + 5 + length - 1];

        match compression {
            2 => {
                let mut decoder = ZlibDecoder::new(compressed);
                let mut out = Vec::new();
                decoder.read_to_end(&mut out).ok()?;
                Some(out)
            }
            _ => None,
        }
    }

    pub fn write_chunk(&self, chunk_x: i32, chunk_z: i32, nbt_data: &[u8]) {
        // compress with zlib
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(nbt_data).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut file_data = if self.path.exists() {
            std::fs::read(&self.path).unwrap_or_else(|_| vec![0u8; 8192])
        } else {
            std::fs::create_dir_all(self.path.parent().unwrap()).ok();
            vec![0u8; 8192]
        };

        // ensure header exists
        if file_data.len() < 8192 {
            file_data.resize(8192, 0);
        }

        // chunkdata: 4 bytes length + 1 byte compression + compressed data
        let chunk_payload_len = compressed.len() + 1; // +1 for compression byte
        let total_len = chunk_payload_len + 4; // +4 for length field
        let sectors_needed = total_len.div_ceil(4096);

        let local_x = chunk_x.rem_euclid(32);
        let local_z = chunk_z.rem_euclid(32);
        let header_offset = (local_x + local_z * 32) as usize * 4;

        let old_offset = {
            let b = &file_data[header_offset..header_offset + 4];
            (b[0] as usize) << 16 | (b[1] as usize) << 8 | b[2] as usize
        };
        let old_sectors = file_data[header_offset + 3] as usize;

        let mut used: HashSet<usize> = HashSet::new();
        for i in 0..1024usize {
            let ho = i * 4;
            let off = (file_data[ho] as usize) << 16
                | (file_data[ho + 1] as usize) << 8
                | file_data[ho + 2] as usize;
            let cnt = file_data[ho + 3] as usize;
            let this_chunks_idx = local_x as usize + local_z as usize * 32;
            if i == this_chunks_idx {
                continue;
            }
            if off >= 2 && cnt > 0 {
                for s in off..off + cnt {
                    used.insert(s);
                }
            }
        }

        let total_file_sectors = (file_data.len() / 4096).max(2);
        let sector_offset = self.find_free_sectors(&used, sectors_needed, total_file_sectors);

        let required_len = (sector_offset + sectors_needed) * 4096;
        if file_data.len() < required_len {
            file_data.resize(required_len, 0);
        }

        if old_offset >= 2 && old_sectors > 0 {
            let old_start = old_offset * 4096;
            let old_end = (old_offset + old_sectors) * 4096;
            if old_end <= file_data.len() {
                file_data[old_start..old_end].fill(0);
            }
        }

        let byte_offset = sector_offset * 4096;
        let mut chunk_bytes = Vec::with_capacity(sectors_needed * 4096);
        chunk_bytes.extend_from_slice(&(chunk_payload_len as u32).to_be_bytes());
        chunk_bytes.push(2); // compression type: zlib
        chunk_bytes.extend_from_slice(&compressed);
        chunk_bytes.resize(sectors_needed * 4096, 0);
        file_data[byte_offset..byte_offset + sectors_needed * 4096].copy_from_slice(&chunk_bytes);

        file_data[header_offset] = ((sector_offset >> 16) & 0xFF) as u8;
        file_data[header_offset + 1] = ((sector_offset >> 8) & 0xFF) as u8;
        file_data[header_offset + 2] = (sector_offset & 0xFF) as u8;
        file_data[header_offset + 3] = sectors_needed as u8;

        // update timestamp (4096-8191)
        let ts_offset = header_offset + 4096;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;
        file_data[ts_offset..ts_offset + 4].copy_from_slice(&timestamp.to_be_bytes());

        let trimmed = self.trim_trailing_empty_sectors(&file_data);

        std::fs::write(&self.path, trimmed).ok();
    }

    #[allow(clippy::mut_range_bound)]
    fn find_free_sectors(
        &self,
        used: &HashSet<usize>,
        count: usize,
        current_total: usize,
    ) -> usize {
        let search_limit = current_total + count + 1;
        let mut start = 2usize;
        'outer: loop {
            if start >= search_limit {
                return start; // append at the end
            }
            for s in start..start + count {
                if used.contains(&s) {
                    start = s + 1;
                    continue 'outer;
                }
            }
            return start;
        }
    }

    fn trim_trailing_empty_sectors<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        let mut end = data.len();
        while end > 8192 {
            let sector_start = end - 4096;
            if data[sector_start..end].iter().all(|&b| b == 0) {
                end = sector_start;
            } else {
                break;
            }
        }
        &data[..end]
    }
}
