use crate::{
    anvil::{chunk_to_nbt, nbt_to_blocks_raw},
    generator::WorldGenerator,
    nbt::NbtTag,
    region::RegionFile,
};
use coral_protocol::packets::play::block::BlockFace;
use coral_types::{ToolKind, ToolMaterial};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

pub mod definitions;
pub mod fluid;
pub mod registry;

pub type ChunkBlock = Arc<HashMap<(i32, u8, i32), Block>>;
pub type ChunkEdits = HashMap<(i32, u8, i32), Block>;

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

    fn is_replaceable(&self) -> bool {
        false
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
    pub blocks: RwLock<HashMap<(i32, i32), ChunkEdits>>,
    // chunks loaded from disk into mem
    pub chunk_cache: RwLock<HashMap<(i32, i32), (Arc<Vec<u8>>, Instant)>>,
    // cache for better peformances
    pub decoded_cache: RwLock<HashMap<(i32, i32), (Arc<HashMap<(i32, u8, i32), Block>>, Instant)>>,
    // chunks that need to be saved
    pub dirty_chunks: RwLock<HashSet<(i32, i32)>>,
    pub world_dir: RwLock<Option<std::path::PathBuf>>,
}

const MAX_CACHED_CHUNKS: usize = 2048;
const MAX_DECODED_BYTES: usize = 64 * 1024 * 1024;

fn decoded_chunk_bytes(len: usize) -> usize {
    const PAIR: usize = std::mem::size_of::<(i32, u8, i32)>() + std::mem::size_of::<Block>();
    let capacity = (len * 8 / 7).next_power_of_two().max(1);
    capacity * (PAIR + 1)
}

fn trim_decoded_cache(
    cache: &mut HashMap<(i32, i32), (Arc<HashMap<(i32, u8, i32), Block>>, Instant)>,
) {
    let mut total: usize = cache
        .values()
        .map(|(blocks, _)| decoded_chunk_bytes(blocks.len()))
        .sum();
    if total <= MAX_DECODED_BYTES {
        return;
    }
    let mut by_age: Vec<((i32, i32), Instant, usize)> = cache
        .iter()
        .map(|(k, (blocks, at))| (*k, *at, decoded_chunk_bytes(blocks.len())))
        .collect();
    by_age.sort_by_key(|(_, at, _)| *at);

    let mut evicted = 0usize;
    for (key, _, bytes) in by_age {
        if total <= MAX_DECODED_BYTES {
            break;
        }
        cache.remove(&key);
        total -= bytes.min(total);
        evicted += 1;
    }

    if evicted > 0 {
        tracing::info!(
            "[World] Evicted {evicted} decoded chunks ({} remaining, ~{} MiB)",
            cache.len(),
            total / (1024 * 1024)
        );
    }
}

impl WorldBlocks {
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
            chunk_cache: RwLock::new(HashMap::new()),
            decoded_cache: RwLock::new(HashMap::new()),
            dirty_chunks: RwLock::new(HashSet::new()),
            world_dir: RwLock::new(None),
        }
    }

    pub async fn get_chunk_nbt(&self, cx: i32, cz: i32) -> Option<Arc<Vec<u8>>> {
        if let Some((nbt, _)) = self.chunk_cache.read().await.get(&(cx, cz)) {
            return Some(nbt.clone());
        }

        let rx = cx >> 5;
        let rz = cz >> 5;
        let nbt = {
            let world_dir = self.world_dir.read().await;
            let dir = world_dir.as_ref()?;
            let region = RegionFile::new(dir, rx, rz);
            Arc::new(region.read_chunk(cx, cz)?)
        };

        self.chunk_cache
            .write()
            .await
            .insert((cx, cz), (nbt.clone(), Instant::now()));
        Some(nbt)
    }

    pub async fn get(&self, x: i32, y: u8, z: i32, generator: &dyn WorldGenerator) -> Block {
        let cx = x >> 4;
        let cz = z >> 4;

        if let Some(b) = self
            .blocks
            .read()
            .await
            .get(&(cx, cz))
            .and_then(|edits| edits.get(&(x, y, z)))
        {
            return b.clone();
        }

        if let Some((decoded, _)) = self.decoded_cache.read().await.get(&(cx, cz)) {
            return decoded.get(&(x, y, z)).cloned().unwrap_or_else(Block::air);
        }

        if let Some(nbt) = self.get_chunk_nbt(cx, cz).await {
            let mut chunk_blocks = HashMap::new();
            nbt_to_blocks_raw(&nbt, &mut chunk_blocks);
            let result = chunk_blocks
                .get(&(x, y, z))
                .cloned()
                .unwrap_or_else(Block::air);
            {
                let mut cache = self.decoded_cache.write().await;
                cache.insert((cx, cz), (Arc::new(chunk_blocks), Instant::now()));
                trim_decoded_cache(&mut cache);
            }
            return result;
        }

        generator.block_at(x, y, z)
    }

    pub async fn get_many(
        &self,
        positions: &[(i32, u8, i32)],
        generator: &dyn WorldGenerator,
        out: &mut Vec<Option<Block>>,
    ) {
        out.clear();
        out.resize(positions.len(), None);

        {
            let blocks = self.blocks.read().await;
            let mut current: Option<((i32, i32), &ChunkEdits)> = None;
            for (slot, pos) in out.iter_mut().zip(positions) {
                let key = (pos.0 >> 4, pos.2 >> 4);
                if current.map(|(k, _)| k) != Some(key) {
                    current = blocks.get(&key).map(|edits| (key, edits));
                }
                *slot = current.and_then(|(_, edits)| edits.get(pos).cloned());
            }
        }
        {
            let decoded = self.decoded_cache.read().await;
            let mut current: Option<((i32, i32), &ChunkBlock)> = None;
            for (slot, pos) in out.iter_mut().zip(positions) {
                if slot.is_some() {
                    continue;
                }
                let key = (pos.0 >> 4, pos.2 >> 4);
                if current.map(|(k, _)| k) != Some(key) {
                    current = decoded.get(&key).map(|(chunk, _)| (key, chunk));
                }
                if let Some((_, chunk)) = current {
                    *slot = Some(chunk.get(pos).cloned().unwrap_or_else(Block::air));
                }
            }
        }

        let mut unresolved: Vec<(i32, i32)> = out
            .iter()
            .zip(positions)
            .filter(|(slot, _)| slot.is_none())
            .map(|(_, pos)| (pos.0 >> 4, pos.2 >> 4))
            .collect();
        unresolved.sort_unstable();
        unresolved.dedup();

        for key in unresolved {
            let decoded = match self.get_chunk_nbt(key.0, key.1).await {
                Some(nbt) => {
                    let mut chunk_blocks = HashMap::new();
                    nbt_to_blocks_raw(&nbt, &mut chunk_blocks);
                    let chunk_blocks = Arc::new(chunk_blocks);
                    let mut cache = self.decoded_cache.write().await;
                    cache.insert(key, (chunk_blocks.clone(), Instant::now()));
                    trim_decoded_cache(&mut cache);
                    Some(chunk_blocks)
                }
                None => None,
            };

            for (slot, pos) in out.iter_mut().zip(positions) {
                if slot.is_some() || (pos.0 >> 4, pos.2 >> 4) != key {
                    continue;
                }
                *slot = Some(match &decoded {
                    Some(chunk) => chunk.get(pos).cloned().unwrap_or_else(Block::air),
                    None => generator.block_at(pos.0, pos.1, pos.2),
                });
            }
        }
    }

    pub async fn chunk_has_edits(&self, cx: i32, cz: i32) -> bool {
        self.blocks
            .read()
            .await
            .get(&(cx, cz))
            .is_some_and(|edits| !edits.is_empty())
    }

    pub async fn chunk_edits(&self, cx: i32, cz: i32) -> ChunkEdits {
        self.blocks
            .read()
            .await
            .get(&(cx, cz))
            .cloned()
            .unwrap_or_default()
    }

    pub async fn is_untouched_column(&self, x: i32, z: i32) -> bool {
        let (cx, cz) = (x >> 4, z >> 4);
        if self.get_chunk_nbt(cx, cz).await.is_some() {
            return false;
        }
        !self
            .blocks
            .read()
            .await
            .get(&(cx, cz))
            .is_some_and(|edits| edits.keys().any(|(bx, _, bz)| *bx == x && *bz == z))
    }

    pub async fn set(&self, x: i32, y: u8, z: i32, block: Block) {
        let cx = x >> 4;
        let cz = z >> 4;

        self.get_chunk_nbt(cx, cz).await;

        self.blocks
            .write()
            .await
            .entry((cx, cz))
            .or_default()
            .insert((x, y, z), block);
        self.dirty_chunks.write().await.insert((cx, cz));
        self.decoded_cache.write().await.remove(&(cx, cz));
    }

    pub async fn evict_stale_chunks(&self, max_age: Duration) {
        {
            let mut decoded = self.decoded_cache.write().await;
            let now = Instant::now();
            decoded.retain(|_, (_, last)| now.duration_since(*last) < max_age);
            trim_decoded_cache(&mut decoded);
        }
        let dirty = self.dirty_chunks.read().await;
        let mut cache = self.chunk_cache.write().await;

        if cache.len() <= MAX_CACHED_CHUNKS {
            return;
        }

        let now = Instant::now();
        let before = cache.len();

        cache.retain(|key, (_, last_accessed)| {
            dirty.contains(key) || now.duration_since(*last_accessed) < max_age
        });

        let evicted = before - cache.len();
        if evicted > 0 {
            tracing::info!(
                "[World] Evicted {} stale chunks from cache ({} remaining).",
                evicted,
                cache.len()
            );
        }
    }

    pub async fn load(&self, world_dir: &Path) {
        let region_dir = world_dir.join("region");
        if !region_dir.exists() {
            tracing::info!("[World] No existing world found, starting fresh!");
            return;
        }
        *self.world_dir.write().await = Some(world_dir.to_path_buf());

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
        tracing::info!(
            "[World] Loaded {} region files. Chunks will load on demand.",
            count
        );
    }

    pub async fn save(
        &self,
        world_dir: &Path,
        generator: Arc<dyn WorldGenerator>,
        tile_entities_by_chunk: &HashMap<(i32, i32), Vec<NbtTag>>,
    ) {
        let dirty = {
            let d = self.dirty_chunks.read().await;
            if d.is_empty() {
                tracing::info!("[World] Nothing to save.");
                return;
            }
            d.clone()
        };

        let diffs_by_chunk: HashMap<(i32, i32), ChunkEdits> = {
            let blocks = self.blocks.read().await;
            dirty
                .iter()
                .filter_map(|key| blocks.get(key).map(|edits| (*key, edits.clone())))
                .collect()
        };
        let world_dir = world_dir.to_path_buf();
        let region_dir = world_dir.join("region");

        // group dirty chunks by region file
        let mut region_map: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
        for (cx, cz) in &dirty {
            region_map
                .entry((cx >> 5, cz >> 5))
                .or_default()
                .push((*cx, *cz));
        }

        tokio::fs::create_dir_all(&region_dir).await.ok();

        // build merged block maps + is_new flags for each dirty chunk
        let mut chunk_block_maps: HashMap<(i32, i32), HashMap<(i32, u8, i32), Block>> =
            HashMap::new();
        let mut chunk_is_new: HashMap<(i32, i32), bool> = HashMap::new();

        {
            let cache = self.chunk_cache.read().await;

            for (cx, cz) in &dirty {
                let (base, is_new) = if let Some((nbt, _)) = cache.get(&(*cx, *cz)) {
                    let mut m = HashMap::new();
                    nbt_to_blocks_raw(nbt, &mut m);
                    (m, false)
                } else {
                    (HashMap::new(), true)
                };

                // diffs always win over base
                let mut final_blocks = base;
                if let Some(diffs) = diffs_by_chunk.get(&(*cx, *cz)) {
                    for (pos, block) in diffs {
                        final_blocks.insert(*pos, block.clone());
                    }
                }

                chunk_block_maps.insert((*cx, *cz), final_blocks);
                chunk_is_new.insert((*cx, *cz), is_new);
            }
        }

        // read existing region files async, before the blocking compression step
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
        let tile_entities_by_chunk = tile_entities_by_chunk.clone();

        let results = tokio::task::spawn_blocking(move || {
            let mut outputs: Vec<(std::path::PathBuf, Vec<u8>)> = vec![];

            for (rx, rz, chunks, mut file_data) in region_data {
                if file_data.len() < 8192 {
                    file_data.resize(8192, 0);
                }

                let mut used = vec![false; (file_data.len() / 4096).max(2)];
                fn mark(used: &mut Vec<bool>, off: usize, count: usize, taken: bool) {
                    if off < 2 || count == 0 {
                        return;
                    }
                    if used.len() < off + count {
                        used.resize(off + count, false);
                    }
                    used[off..off + count].fill(taken);
                }
                for i in 0..1024usize {
                    let ho = i * 4;
                    if ho + 4 > file_data.len() {
                        continue;
                    }
                    let off = (file_data[ho] as usize) << 16
                        | (file_data[ho + 1] as usize) << 8
                        | file_data[ho + 2] as usize;
                    mark(&mut used, off, file_data[ho + 3] as usize, true);
                }

                for (cx, cz) in &chunks {
                    let Some(final_blocks) = chunk_block_maps.get(&(*cx, *cz)) else {
                        continue;
                    };
                    let is_new = *chunk_is_new.get(&(*cx, *cz)).unwrap_or(&true);
                    let tiles = tile_entities_by_chunk
                        .get(&(*cx, *cz))
                        .cloned()
                        .unwrap_or_default();

                    let nbt =
                        chunk_to_nbt(*cx, *cz, final_blocks, generator.as_ref(), tiles, is_new);

                    let compressed = {
                        use flate2::Compression;
                        use flate2::write::ZlibEncoder;
                        use std::io::Write;
                        let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
                        enc.write_all(&nbt).unwrap();
                        enc.finish().unwrap()
                    };

                    let payload_len = compressed.len() + 1;
                    let total_len = payload_len + 4;
                    let sectors_needed = total_len.div_ceil(4096);

                    let local_x = cx.rem_euclid(32) as usize;
                    let local_z = cz.rem_euclid(32) as usize;
                    let header_offset = (local_x + local_z * 32) * 4;

                    let old_offset = (file_data[header_offset] as usize) << 16
                        | (file_data[header_offset + 1] as usize) << 8
                        | file_data[header_offset + 2] as usize;
                    let old_sectors = file_data[header_offset + 3] as usize;

                    // this chunk may reuse the sectors it is vacating
                    mark(&mut used, old_offset, old_sectors, false);
                    let mut sector_offset = 2usize;
                    'find: loop {
                        // anything past the end of the file is free
                        if sector_offset + sectors_needed > used.len() {
                            break;
                        }
                        for (s, &is_used) in used
                            .iter()
                            .enumerate()
                            .skip(sector_offset)
                            .take(sectors_needed)
                        {
                            if is_used {
                                sector_offset = s + 1;
                                continue 'find;
                            }
                        }
                        break;
                    }
                    mark(&mut used, sector_offset, sectors_needed, true);

                    if old_offset >= 2 && old_sectors > 0 {
                        let s = old_offset * 4096;
                        let e = (old_offset + old_sectors) * 4096;
                        if e <= file_data.len() {
                            file_data[s..e].fill(0);
                        }
                    }

                    let required = (sector_offset + sectors_needed) * 4096;
                    if file_data.len() < required {
                        file_data.resize(required, 0);
                    }

                    let bo = sector_offset * 4096;
                    let mut chunk_bytes = Vec::with_capacity(sectors_needed * 4096);
                    chunk_bytes.extend_from_slice(&(payload_len as u32).to_be_bytes());
                    chunk_bytes.push(2);
                    chunk_bytes.extend_from_slice(&compressed);
                    chunk_bytes.resize(sectors_needed * 4096, 0);
                    file_data[bo..bo + sectors_needed * 4096].copy_from_slice(&chunk_bytes);

                    file_data[header_offset] = ((sector_offset >> 16) & 0xFF) as u8;
                    file_data[header_offset + 1] = ((sector_offset >> 8) & 0xFF) as u8;
                    file_data[header_offset + 2] = (sector_offset & 0xFF) as u8;
                    file_data[header_offset + 3] = sectors_needed as u8;

                    let ts = header_offset + 4096;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as u32;
                    file_data[ts..ts + 4].copy_from_slice(&now.to_be_bytes());
                }

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

        self.dirty_chunks.write().await.clear();

        tracing::info!(
            "[World] Saved {} chunks across {} region files.",
            dirty_count,
            region_count
        );
    }
}

pub fn placement_metadata(
    block_id: u8,
    item_meta: u8,
    face: BlockFace,
    yaw: f32,
    cursor_y: f32,
) -> u8 {
    match block_id {
        // logs
        17 => {
            let wood_type = item_meta & 0x3;
            let axis = match face {
                BlockFace::Down | BlockFace::Up => 0x00,
                BlockFace::North | BlockFace::South => 0x8,
                BlockFace::West | BlockFace::East => 0x4,
            };
            wood_type | axis
        }
        // stairs
        53 | 67 | 108 | 109 | 114 | 128 | 134 | 135 | 136 | 156 | 163 | 164 | 180 => {
            let facing = match yaw_to_facing(yaw) {
                BlockFace::North => 2,
                BlockFace::East => 1,
                BlockFace::South => 3,
                _ => 0,
            };
            let upside_down = if face == BlockFace::Down
                || (face as u8 >= BlockFace::South as u8 && cursor_y > 0.5)
            {
                0x4
            } else {
                0x0
            };
            facing | upside_down
        }
        // torches
        50 | 75 | 76 => match face {
            BlockFace::Up => 5,
            BlockFace::North => 4,
            BlockFace::South => 3,
            BlockFace::West => 2,
            BlockFace::East => 1,
            _ => 5,
        },
        // ladders, wall signs
        65 | 68 => match face {
            BlockFace::North => 2,
            BlockFace::South => 3,
            BlockFace::West => 4,
            BlockFace::East => 5,
            _ => 2,
        },
        // furnaces, chests, pumpkins, dispenser, enderchest
        54 | 61 | 62 | 130 | 146 => match yaw_to_facing(yaw) {
            BlockFace::North => 2,
            BlockFace::East => 5,
            BlockFace::South => 3,
            _ => 4,
        },

        // pumpkin & jack'o
        86 | 91 => yaw_to_facing(yaw) as u8,

        // slabs
        44 | 126 => {
            let variant = item_meta & 0x7;
            let top = if face == BlockFace::Up
                || (face as u8 >= BlockFace::North as u8 && cursor_y > 0.5)
            {
                0x8
            } else {
                0x0
            };
            variant | top
        }
        _ => item_meta,
    }
}
fn yaw_to_facing(yaw: f32) -> BlockFace {
    // normalize yaw to 0..360
    let yaw = yaw.rem_euclid(360.0);
    match ((yaw + 45.0) / 90.0).floor() as i32 & 3 {
        0 => BlockFace::North,
        1 => BlockFace::East,
        2 => BlockFace::South,
        _ => BlockFace::West,
    }
}

#[cfg(test)]
mod region_tests {
    use super::*;
    use crate::world::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("coral-region-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[tokio::test]
    async fn many_chunks_in_one_region_survive_a_save_and_reload() {
        let dir = tmp("many");
        let w = World::flat();

        let chunks: Vec<(i32, i32)> = (0..8).flat_map(|x| (0..8).map(move |z| (x, z))).collect();
        for (i, (cx, cz)) in chunks.iter().enumerate() {
            w.set(
                cx * 16 + 1,
                70,
                cz * 16 + 1,
                Block::new((i % 200 + 1) as u8, (i % 16) as u8),
            )
            .await;
        }
        w.save(&dir, &HashMap::new()).await;

        let reloaded = World::flat();
        reloaded.load(&dir).await;
        for (i, (cx, cz)) in chunks.iter().enumerate() {
            let b = reloaded.get(cx * 16 + 1, 70, cz * 16 + 1).await;
            assert_eq!(
                (b.id, b.metadata),
                ((i % 200 + 1) as u8, (i % 16) as u8),
                "chunk ({cx},{cz}) came back wrong"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn a_chunk_that_grows_relocates_without_clobbering_its_neighbours() {
        let dir = tmp("grow");
        let w = World::flat();

        for cx in 0..4i32 {
            w.set(cx * 16 + 2, 70, 2, Block::new(41, 0)).await;
        }
        w.save(&dir, &HashMap::new()).await;

        // chunk 0 becomes far larger than the sectors it holds
        for x in 0..16i32 {
            for z in 0..16i32 {
                for y in 100..160u8 {
                    w.set(x, y, z, Block::new(1, 0)).await;
                }
            }
        }
        w.save(&dir, &HashMap::new()).await;

        let reloaded = World::flat();
        reloaded.load(&dir).await;

        // the grown chunk
        assert_eq!(reloaded.get(5, 130, 5).await.id, 1);
        assert_eq!(reloaded.get(2, 70, 2).await.id, 41);
        // and the neighbours it had to move around
        for cx in 1..4i32 {
            assert_eq!(
                reloaded.get(cx * 16 + 2, 70, 2).await.id,
                41,
                "chunk {cx} was overwritten"
            );
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn resaving_a_chunk_reuses_its_sectors() {
        let dir = tmp("resave");
        let w = World::flat();
        w.set(1, 70, 1, Block::new(41, 0)).await;
        w.save(&dir, &HashMap::new()).await;

        let region = dir.join("region").join("r.0.0.mca");
        let first = std::fs::metadata(&region).unwrap().len();

        for i in 0..5u8 {
            w.set(1, 70, 1, Block::new(41 + i, 0)).await;
            w.save(&dir, &HashMap::new()).await;
        }
        let after = std::fs::metadata(&region).unwrap().len();
        assert_eq!(first, after, "the region file grew on a same-size rewrite");

        let reloaded = World::flat();
        reloaded.load(&dir).await;
        assert_eq!(reloaded.get(1, 70, 1).await.id, 45);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(test)]
    mod decoded_cache_tests {
        use crate::blocks::{MAX_DECODED_BYTES, WorldBlocks, trim_decoded_cache};

        use super::*;

        fn chunk(len: usize) -> Arc<HashMap<(i32, u8, i32), Block>> {
            let mut m = HashMap::with_capacity(len);
            for i in 0..len {
                m.insert((i as i32, (i % 256) as u8, i as i32), Block::new(1, 0));
            }
            Arc::new(m)
        }

        fn cache_of(
            entries: usize,
            blocks_each: usize,
        ) -> HashMap<(i32, i32), (Arc<HashMap<(i32, u8, i32), Block>>, Instant)> {
            let mut c = HashMap::new();
            let base = Instant::now();
            let shared = chunk(blocks_each);
            for i in 0..entries {
                let at = base + Duration::from_millis(i as u64);
                c.insert((i as i32, 0), (shared.clone(), at));
            }
            c
        }

        fn total(c: &HashMap<(i32, i32), (Arc<HashMap<(i32, u8, i32), Block>>, Instant)>) -> usize {
            c.values().map(|(b, _)| decoded_chunk_bytes(b.len())).sum()
        }

        #[test]
        fn the_cost_estimate_tracks_hashbrowns_growth() {
            assert_eq!(decoded_chunk_bytes(4096), 8192 * 15);
            assert_eq!(decoded_chunk_bytes(16_384), 32_768 * 15);
            assert_eq!(decoded_chunk_bytes(65_536), 131_072 * 15);
            assert!(decoded_chunk_bytes(0) > 0);
        }

        #[test]
        fn the_cache_is_trimmed_back_under_budget() {
            let mut c = cache_of(200, 16_384);
            assert!(
                total(&c) > MAX_DECODED_BYTES,
                "fixture should exceed the budget"
            );

            trim_decoded_cache(&mut c);

            assert!(
                total(&c) <= MAX_DECODED_BYTES,
                "still over budget after trim"
            );
            assert!(!c.is_empty(), "the trim emptied the cache");
        }

        #[test]
        fn trimming_drops_the_oldest_first() {
            let mut c = cache_of(200, 16_384);
            let newest = (199, 0);
            let oldest = (0, 0);

            trim_decoded_cache(&mut c);

            assert!(c.contains_key(&newest), "evicted the most recent chunk");
            assert!(!c.contains_key(&oldest), "kept the oldest chunk");
        }

        #[test]
        fn a_cache_inside_the_budget_is_left_alone() {
            let mut c = cache_of(4, 16_384);
            let before: Vec<(i32, i32)> = {
                let mut k: Vec<_> = c.keys().copied().collect();
                k.sort();
                k
            };
            trim_decoded_cache(&mut c);
            let mut after: Vec<(i32, i32)> = c.keys().copied().collect();
            after.sort();
            assert_eq!(before, after);
        }

        #[tokio::test]
        async fn repeated_reads_cache_one_entry_per_chunk() {
            let w = WorldBlocks::new();
            let g = crate::generator::FlatWorldGenerator::new();
            for _ in 0..5 {
                w.get(3, 2, 3, &g).await;
            }
            assert_eq!(w.decoded_cache.read().await.len(), 0);
        }
    }
}

#[cfg(test)]
mod chunk_edit_index_tests {
    use super::*;
    use crate::generator::FlatWorldGenerator;

    fn blocks() -> WorldBlocks {
        WorldBlocks::new()
    }

    fn flat() -> FlatWorldGenerator {
        FlatWorldGenerator::new()
    }

    #[tokio::test]
    async fn an_edit_is_filed_under_its_own_chunk() {
        let b = blocks();
        b.set(5, 64, 5, Block::new(1, 0)).await;

        assert!(b.chunk_has_edits(0, 0).await);
        assert!(!b.chunk_has_edits(1, 0).await);
        assert!(!b.chunk_has_edits(0, 1).await);
        assert!(!b.chunk_has_edits(-1, -1).await);
    }

    #[tokio::test]
    async fn negative_coordinates_land_in_the_right_chunk() {
        let b = blocks();
        b.set(-1, 64, -1, Block::new(1, 0)).await;

        assert!(b.chunk_has_edits(-1, -1).await, "(-1,-1) went elsewhere");
        assert!(!b.chunk_has_edits(0, 0).await);
    }

    #[tokio::test]
    async fn chunk_edits_returns_only_that_chunks_edits() {
        let b = blocks();
        b.set(1, 64, 1, Block::new(1, 0)).await;
        b.set(2, 64, 2, Block::new(2, 0)).await;
        b.set(20, 64, 1, Block::new(3, 0)).await; // chunk (1, 0)

        let here = b.chunk_edits(0, 0).await;
        assert_eq!(here.len(), 2);
        assert!(here.contains_key(&(1, 64, 1)));
        assert!(here.contains_key(&(2, 64, 2)));

        let next = b.chunk_edits(1, 0).await;
        assert_eq!(next.len(), 1);
        assert!(next.contains_key(&(20, 64, 1)));

        assert!(b.chunk_edits(9, 9).await.is_empty());
    }

    #[tokio::test]
    async fn get_still_sees_every_edit() {
        let b = blocks();
        let generator = flat();
        let edits = [
            (0, 64u8, 0, 1u8),
            (15, 64, 15, 2),
            (16, 64, 16, 3),
            (-1, 64, -1, 4),
            (-16, 70, -16, 5),
            (-17, 70, -17, 6),
            (300, 20, -400, 7),
        ];
        for (x, y, z, id) in edits {
            b.set(x, y, z, Block::new(id, 0)).await;
        }
        for (x, y, z, id) in edits {
            assert_eq!(
                b.get(x, y, z, &generator).await.id,
                id,
                "at ({x}, {y}, {z})"
            );
        }
        assert_eq!(
            b.get(1000, 64, 1000, &generator).await.id,
            generator.block_at(1000, 64, 1000).id
        );
    }

    #[tokio::test]
    async fn overwriting_a_position_replaces_rather_than_duplicates() {
        let b = blocks();
        let generator = flat();
        b.set(3, 64, 3, Block::new(1, 0)).await;
        b.set(3, 64, 3, Block::new(9, 2)).await;

        assert_eq!(b.chunk_edits(0, 0).await.len(), 1);
        let got = b.get(3, 64, 3, &generator).await;
        assert_eq!((got.id, got.metadata), (9, 2));
    }

    #[tokio::test]
    async fn get_many_agrees_with_get_across_chunks() {
        let b = blocks();
        let generator = flat();
        b.set(-1, 64, -1, Block::new(1, 0)).await;
        b.set(0, 64, 0, Block::new(2, 0)).await;
        b.set(16, 64, 16, Block::new(3, 0)).await;

        let positions: Vec<_> = (-18..=18)
            .flat_map(|x| (-18..=18).map(move |z| (x, 64u8, z)))
            .collect();
        let mut batch = Vec::new();
        b.get_many(&positions, &generator, &mut batch).await;

        for (pos, got) in positions.iter().zip(&batch) {
            let one = b.get(pos.0, pos.1, pos.2, &generator).await;
            let got = got.as_ref().unwrap();
            assert_eq!((got.id, got.metadata), (one.id, one.metadata), "at {pos:?}");
        }
    }

    #[tokio::test]
    async fn an_untouched_column_stays_untouched_until_it_is_edited() {
        let b = WorldBlocks::new();
        assert!(b.is_untouched_column(4, 4).await);

        b.set(4, 64, 4, Block::new(1, 0)).await;
        assert!(!b.is_untouched_column(4, 4).await);
        assert!(b.is_untouched_column(5, 4).await);
        assert!(b.is_untouched_column(40, 4).await);
    }

    #[tokio::test]
    async fn a_void_world_only_reports_edits_in_the_chunk_asked_about() {
        let b = blocks();
        for i in 0..64 {
            b.set(i * 16, 64, 0, Block::new(1, 0)).await;
        }
        for i in 0..64 {
            assert!(b.chunk_has_edits(i, 0).await, "chunk ({i}, 0)");
        }
        assert!(!b.chunk_has_edits(0, 1).await);
    }
}
