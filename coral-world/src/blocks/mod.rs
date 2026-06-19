use crate::{
    anvil::{chunk_to_nbt, nbt_to_blocks, nbt_to_blocks_raw},
    generator::{self, FlatWorldGenerator},
    region::RegionFile,
};
use coral_types::{ToolKind, ToolMaterial};
use flate2::read::ZlibDecoder;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
};
use tokio::sync::RwLock;

pub mod definitions;
pub mod registry;

pub trait BlockBehavior: Send + Sync {
    fn id(&self) -> u8;

    /// Hardness — time in seconds to break by hand.
    /// 0.0 = instant, f32::INFINITY = unbreakable (bedrock).
    fn hardness(&self) -> f32;

    /// Which tool kind is required/preferred.
    fn required_tool(&self) -> ToolKind {
        ToolKind::None
    }

    /// Minimum tool material required to drop anything.
    /// e.g. diamond ore needs iron+ or it drops nothing.
    fn required_material(&self) -> Option<ToolMaterial> {
        None
    }

    /// Whether the block drops itself or delegates to block_drop().
    fn drops_self(&self) -> bool {
        true
    }

    /// Whether this block is solid (used for collision, placement checks).
    fn is_solid(&self) -> bool {
        true
    }

    /// (used for light)
    fn is_transparent(&self) -> bool {
        false
    }

    fn is_flammable(&self) -> bool {
        false
    }

    /// Light level emitted by this block (0-15).
    fn light_emission(&self) -> u8 {
        0
    }

    fn blast_resistance(&self) -> f32 {
        self.hardness() * 5.0
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: u8,
    pub metadata: u8,
}

impl Block {
    pub fn new(id: u8, metadata: u8) -> Self {
        Self { id, metadata }
    }

    pub fn air() -> Self {
        Self { id: 0, metadata: 0 }
    }

    pub fn is_air(&self) -> bool {
        self.id == 0
    }
}

pub struct WorldBlocks {
    // only player modified blocks
    pub blocks: RwLock<HashMap<(i32, u8, i32), Block>>,
    // chunks loaded from disk into mem
    pub chunk_cache: RwLock<HashMap<(i32, i32), Vec<u8>>>,
    // chunks that need to be saved
    pub dirty_chunks: RwLock<HashSet<(i32, i32)>>,
    pub world_dir: RwLock<Option<std::path::PathBuf>>,
    pub generator: RwLock<Option<FlatWorldGenerator>>,
}
impl WorldBlocks {
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
            chunk_cache: RwLock::new(HashMap::new()),
            dirty_chunks: RwLock::new(HashSet::new()),
            world_dir: RwLock::new(None),
            generator: RwLock::new(None),
        }
    }

    pub async fn get_chunk_nbt(&self, cx: i32, cz: i32) -> Option<Vec<u8>> {
        if let Some(nbt) = self.chunk_cache.read().await.get(&(cx, cz)) {
            return Some(nbt.clone());
        }

        let rx = cx >> 5;
        let rz = cz >> 5;
        let world_dir = self.world_dir.read().await;
        let Some(dir) = world_dir.as_ref() else {
            return None;
        };
        let region = RegionFile::new(dir, rx, rz);
        let nbt = region.read_chunk(cx, cz)?;

        self.chunk_cache.write().await.insert((cx, cz), nbt.clone());
        Some(nbt)
    }

    pub async fn get(&self, x: i32, y: u8, z: i32, generator: &FlatWorldGenerator) -> Block {
        if let Some(b) = self.blocks.read().await.get(&(x, y, z)) {
            return b.clone();
        }

        let cx = x >> 4;
        let cz = z >> 4;

        if let Some(nbt) = self.get_chunk_nbt(cx, cz).await {
            let mut chunk_blocks = HashMap::new();
            nbt_to_blocks_raw(&nbt, &mut chunk_blocks);
            if let Some(b) = chunk_blocks.get(&(x, y, z)) {
                return b.clone();
            }
            return Block::air();
        }

        generator.get(y)
    }

    pub async fn set(&self, x: i32, y: u8, z: i32, block: Block, generator: &FlatWorldGenerator) {
        let gen_block = generator.get(y);
        let cx = x >> 4;
        let cz = z >> 4;

        // TODO: remove this shit
        {
            let mut blocks = self.blocks.write().await;
            if block.id == gen_block.id && block.metadata == gen_block.metadata {
                blocks.remove(&(x, y, z));
            } else {
                blocks.insert((x, y, z), block);
            }
        }
        self.dirty_chunks.write().await.insert((cx, cz));
    }

    pub async fn load(&self, world_dir: &Path, generator: &FlatWorldGenerator) {
        let region_dir = world_dir.join("region");
        if !region_dir.exists() {
            println!("[World] No existing world found, starting fresh!");
            return;
        }
        *self.world_dir.write().await = Some(world_dir.to_path_buf());
        *self.generator.write().await = Some(generator.clone());

        let count = std::fs::read_dir(&region_dir)
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_name()
                            .to_str()
                            .map(|n| n.ends_with(".mca"))
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);
        println!(
            "[World] Loaded {} region files. Chunks will load on demand.",
            count
        );
    }

    pub async fn save(&self, world_dir: &Path, generator: &FlatWorldGenerator) {
        let dirty = {
            let d = self.dirty_chunks.read().await;
            if d.is_empty() {
                println!("[World] Nothing to save.");
                return;
            }
            d.clone()
        };

        let saved = self.chunk_cache.read().await.clone();
        let blocks = self.blocks.read().await.clone();
        let generator = generator.clone();
        let world_dir = world_dir.to_path_buf();
        let region_dir = world_dir.join("region");

        // group dirty chunks by region
        let mut region_map: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
        for (cx, cz) in &dirty {
            region_map
                .entry((cx >> 5, cz >> 5))
                .or_default()
                .push((*cx, *cz));
        }

        // read existing region files async
        tokio::fs::create_dir_all(&region_dir).await.ok();

        let mut region_data: Vec<(i32, i32, Vec<(i32, i32)>, Vec<u8>)> = vec![];
        for ((rx, rz), chunks) in region_map {
            let region_path = region_dir.join(format!("r.{}.{}.mca", rx, rz));
            let file_data = tokio::fs::read(&region_path)
                .await
                .unwrap_or_else(|_| vec![0u8; 8192]);
            region_data.push((rx, rz, chunks, file_data));
        }

        let region_dir_clone = region_dir.clone();
        let dirty_count = dirty.len();

        let results = tokio::task::spawn_blocking(move || {
            let mut outputs: Vec<(std::path::PathBuf, Vec<u8>)> = vec![];

            for (rx, rz, chunks, mut file_data) in region_data {
                if file_data.len() < 8192 {
                    file_data.resize(8192, 0);
                }

                for (cx, cz) in &chunks {
                    let is_new = !saved.contains_key(&(*cx, *cz));
                    let nbt = chunk_to_nbt(*cx, *cz, &blocks, &generator, is_new);

                    let compressed = {
                        use flate2::Compression;
                        use flate2::write::ZlibEncoder;
                        use std::io::Write;
                        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
                        enc.write_all(&nbt).unwrap();
                        enc.finish().unwrap()
                    };

                    let payload_len = compressed.len() + 1; // +1 compression byte
                    let total_len = payload_len + 4; // +4 length field
                    let sectors_needed = total_len.div_ceil(4096);

                    let local_x = cx.rem_euclid(32) as usize;
                    let local_z = cz.rem_euclid(32) as usize;
                    let header_offset = (local_x + local_z * 32) * 4;
                    let this_idx = local_x + local_z * 32;

                    // free old sectors
                    let old_offset = (file_data[header_offset] as usize) << 16
                        | (file_data[header_offset + 1] as usize) << 8
                        | file_data[header_offset + 2] as usize;
                    let old_sectors = file_data[header_offset + 3] as usize;

                    // build used set
                    let mut used: HashSet<usize> = HashSet::new();
                    for i in 0..1024usize {
                        if i == this_idx {
                            continue;
                        }
                        let ho = i * 4;
                        if ho + 4 > file_data.len() {
                            continue;
                        }
                        let off = (file_data[ho] as usize) << 16
                            | (file_data[ho + 1] as usize) << 8
                            | file_data[ho + 2] as usize;
                        let cnt = file_data[ho + 3] as usize;
                        if off >= 2 && cnt > 0 {
                            for s in off..off + cnt {
                                used.insert(s);
                            }
                        }
                    }

                    // find free run
                    let mut sector_offset = 2usize;
                    'find: loop {
                        for s in sector_offset..sector_offset + sectors_needed {
                            if used.contains(&s) {
                                sector_offset = s + 1;
                                continue 'find;
                            }
                        }
                        break;
                    }

                    // zero old sectors
                    if old_offset >= 2 && old_sectors > 0 {
                        let s = old_offset * 4096;
                        let e = (old_offset + old_sectors) * 4096;
                        if e <= file_data.len() {
                            file_data[s..e].fill(0);
                        }
                    }

                    // extend and write
                    let required = (sector_offset + sectors_needed) * 4096;
                    if file_data.len() < required {
                        file_data.resize(required, 0);
                    }

                    let bo = sector_offset * 4096;
                    let mut chunk_bytes = Vec::with_capacity(sectors_needed * 4096);
                    chunk_bytes.extend_from_slice(&(payload_len as u32).to_be_bytes());
                    chunk_bytes.push(2); // zlib
                    chunk_bytes.extend_from_slice(&compressed);
                    chunk_bytes.resize(sectors_needed * 4096, 0);
                    file_data[bo..bo + sectors_needed * 4096].copy_from_slice(&chunk_bytes);

                    // update header
                    file_data[header_offset] = ((sector_offset >> 16) & 0xFF) as u8;
                    file_data[header_offset + 1] = ((sector_offset >> 8) & 0xFF) as u8;
                    file_data[header_offset + 2] = (sector_offset & 0xFF) as u8;
                    file_data[header_offset + 3] = sectors_needed as u8;

                    // timestamp
                    let ts = header_offset + 4096;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as u32;
                    file_data[ts..ts + 4].copy_from_slice(&now.to_be_bytes());
                }

                // trim trailing empty sectors
                let mut end = file_data.len();
                while end > 8192 {
                    let start = end - 4096;
                    if file_data[start..end].iter().all(|&b| b == 0) {
                        end = start;
                    } else {
                        break;
                    }
                }
                file_data.truncate(end);

                outputs.push((
                    region_dir_clone.join(format!("r.{}.{}.mca", rx, rz)),
                    file_data,
                ));
            }

            outputs
        })
        .await
        .unwrap_or_default();

        let region_count = results.len();
        for (path, data) in results {
            tokio::fs::write(&path, &data).await.ok();
        }

        // clear dirty after successful save
        self.dirty_chunks.write().await.clear();

        println!(
            "[World] Saved {} chunks across {} region files.",
            dirty_count, region_count
        );
    }
}
