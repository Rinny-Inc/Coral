use std::collections::{HashMap, HashSet};

use coral_protocol::packets::play::entity::TileEntity;

pub type TilePos = (i32, i32, i32);

pub fn chunk_of(pos: TilePos) -> (i32, i32) {
    (pos.0 >> 4, pos.2 >> 4)
}

#[derive(Debug, Default)]
pub struct TileEntityStore {
    by_chunk: HashMap<(i32, i32), HashMap<TilePos, TileEntity>>,
    /// Position of `TileEntity::Furnace` entries for the furnace tick
    furnaces: HashSet<TilePos>,
    len: usize,
}
impl TileEntityStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of chunks holding at least one tile entity
    pub fn chunk_count(&self) -> usize {
        self.by_chunk.len()
    }

    pub fn get(&self, pos: &TilePos) -> Option<&TileEntity> {
        self.by_chunk.get(&chunk_of(*pos))?.get(pos)
    }

    pub fn get_mut(&mut self, pos: &TilePos) -> Option<&mut TileEntity> {
        self.by_chunk.get_mut(&chunk_of(*pos))?.get_mut(pos)
    }

    pub fn contains(&self, pos: &TilePos) -> bool {
        self.get(pos).is_some()
    }

    pub fn insert(&mut self, pos: TilePos, tile: TileEntity) -> Option<TileEntity> {
        if matches!(tile, TileEntity::Furnace { .. }) {
            self.furnaces.insert(pos);
        } else {
            self.furnaces.remove(&pos);
        }

        let previous = self
            .by_chunk
            .entry(chunk_of(pos))
            .or_default()
            .insert(pos, tile);

        if previous.is_none() {
            self.len += 1;
        }
        previous
    }

    pub fn remove(&mut self, pos: &TilePos) -> Option<TileEntity> {
        let chunk_key = chunk_of(*pos);
        let chunk = self.by_chunk.get_mut(&chunk_key)?;
        let removed = chunk.remove(pos);

        if removed.is_some() {
            self.len -= 1;
            self.furnaces.remove(pos);
            if chunk.is_empty() {
                self.by_chunk.remove(&chunk_key);
            }
        }
        removed
    }

    pub fn get_or_insert(&mut self, pos: TilePos, default: TileEntity) -> &mut TileEntity {
        if !self.contains(&pos) {
            self.insert(pos, default);
        }
        self.get_mut(&pos).expect("just inserted")
    }

    pub fn in_chunk(&self, cx: i32, cz: i32) -> impl Iterator<Item = (&TilePos, &TileEntity)> {
        self.by_chunk
            .get(&(cx, cz))
            .into_iter()
            .flat_map(|chunk| chunk.iter())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&TilePos, &TileEntity)> {
        self.by_chunk.values().flat_map(|chunk| chunk.iter())
    }

    pub fn chunks(&self) -> impl Iterator<Item = (&(i32, i32), &HashMap<TilePos, TileEntity>)> {
        self.by_chunk.iter()
    }

    pub fn furnace_positions(&self) -> impl Iterator<Item = TilePos> {
        self.furnaces.iter().copied()
    }

    pub fn furnace_count(&self) -> usize {
        self.furnaces.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(text: &str) -> TileEntity {
        TileEntity::Sign {
            lines: [
                text.to_string(),
                String::new(),
                String::new(),
                String::new(),
            ],
        }
    }

    fn furnace() -> TileEntity {
        TileEntity::Furnace {
            input: None,
            fuel: None,
            output: None,
            burn_ticks: 0,
            burn_ticks_total: 0,
            cook_ticks: 0,
            viewers: 0,
        }
    }

    #[test]
    fn negative_coordinates_floor_into_the_right_chunk() {
        assert_eq!(chunk_of((0, 64, 0)), (0, 0));
        assert_eq!(chunk_of((15, 64, 15)), (0, 0));
        assert_eq!(chunk_of((16, 64, 16)), (1, 1));
        assert_eq!(chunk_of((-1, 64, -1)), (-1, -1));
        assert_eq!(chunk_of((-16, 64, -16)), (-1, -1));
        assert_eq!(chunk_of((-17, 64, -17)), (-2, -2));
    }

    #[test]
    fn a_tile_is_found_by_position_and_by_chunk() {
        let mut store = TileEntityStore::new();
        store.insert((20, 64, -3), sign("hello"));

        assert!(store.get(&(20, 64, -3)).is_some());
        assert_eq!(store.in_chunk(1, -1).count(), 1);
        assert_eq!(store.in_chunk(0, 0).count(), 0, "wrong chunk");
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn in_chunk_returns_only_that_chunks_tiles() {
        let mut store = TileEntityStore::new();
        store.insert((1, 64, 1), sign("a"));
        store.insert((2, 70, 3), sign("b"));
        store.insert((20, 64, 1), sign("c"));
        store.insert((-1, 64, -1), sign("d"));

        let mut origin: Vec<&TilePos> = store.in_chunk(0, 0).map(|(pos, _)| pos).collect();
        origin.sort();
        assert_eq!(origin, vec![&(1, 64, 1), &(2, 70, 3)]);

        assert_eq!(store.in_chunk(1, 0).count(), 1);
        assert_eq!(store.in_chunk(-1, -1).count(), 1);
        assert_eq!(store.len(), 4);
    }

    #[test]
    fn removing_the_last_tile_drops_the_chunk() {
        let mut store = TileEntityStore::new();
        store.insert((100, 64, 100), sign("x"));
        store.insert((101, 64, 100), sign("y"));
        assert_eq!(store.chunk_count(), 1);

        store.remove(&(100, 64, 100));
        assert_eq!(store.chunk_count(), 1, "one tile still there");

        store.remove(&(101, 64, 100));
        assert_eq!(store.chunk_count(), 0, "empty chunk map left behind");
        assert!(store.is_empty());
    }

    #[test]
    fn replacing_a_tile_does_not_double_count_it() {
        let mut store = TileEntityStore::new();
        store.insert((5, 64, 5), sign("first"));
        let previous = store.insert((5, 64, 5), sign("second"));

        assert!(previous.is_some(), "the old tile should be handed back");
        assert_eq!(store.len(), 1);
        assert_eq!(store.in_chunk(0, 0).count(), 1);
    }

    #[test]
    fn removing_something_that_is_not_there_changes_nothing() {
        let mut store = TileEntityStore::new();
        store.insert((5, 64, 5), sign("x"));

        assert!(store.remove(&(6, 64, 5)).is_none(), "same chunk, no tile");
        assert!(store.remove(&(600, 64, 500)).is_none(), "no such chunk");
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn the_furnace_index_tracks_inserts_and_removes() {
        let mut store = TileEntityStore::new();
        store.insert((0, 64, 0), furnace());
        store.insert((1, 64, 0), sign("not a furnace"));
        store.insert((2, 64, 0), furnace());

        let mut found: Vec<TilePos> = store.furnace_positions().collect();
        found.sort();
        assert_eq!(found, vec![(0, 64, 0), (2, 64, 0)]);

        store.remove(&(0, 64, 0));
        assert_eq!(store.furnace_count(), 1);

        store.insert((2, 64, 0), sign("replaced"));
        assert_eq!(store.furnace_count(), 0);
    }

    #[test]
    fn the_furnace_index_survives_mutation_through_get_mut() {
        let mut store = TileEntityStore::new();
        store.insert((0, 64, 0), furnace());

        if let Some(TileEntity::Furnace { burn_ticks, .. }) = store.get_mut(&(0, 64, 0)) {
            *burn_ticks = 200;
        } else {
            panic!("expected a furnace");
        }

        assert_eq!(store.furnace_count(), 1);
        assert!(matches!(
            store.get(&(0, 64, 0)),
            Some(TileEntity::Furnace {
                burn_ticks: 200,
                ..
            })
        ));
    }

    #[test]
    fn get_or_insert_only_inserts_when_absent() {
        let mut store = TileEntityStore::new();

        store.get_or_insert((7, 64, 7), sign("created"));
        assert_eq!(store.len(), 1);

        store.get_or_insert((7, 64, 7), sign("should not replace"));
        assert_eq!(store.len(), 1);
        match store.get(&(7, 64, 7)) {
            Some(TileEntity::Sign { lines }) => assert_eq!(lines[0], "created"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn iter_and_chunks_agree_with_len() {
        let mut store = TileEntityStore::new();
        for i in 0..50i32 {
            store.insert((i * 7, 64, i * 13), sign("x"));
        }

        assert_eq!(store.iter().count(), store.len());
        let via_chunks: usize = store.chunks().map(|(_, tiles)| tiles.len()).sum();
        assert_eq!(via_chunks, store.len());
    }

    #[test]
    fn a_chunk_lookup_does_not_grow_with_the_rest_of_the_world() {
        let mut store = TileEntityStore::new();
        store.insert((0, 64, 0), sign("target"));

        for i in 1..20_000i32 {
            store.insert((i * 16, 64, i * 16), sign("elsewhere"));
        }

        assert_eq!(store.len(), 20_000);
        assert_eq!(
            store.in_chunk(0, 0).count(),
            1,
            "the origin chunk still holds exactly one tile"
        );
        assert_eq!(store.chunk_count(), 20_000);
    }
}
