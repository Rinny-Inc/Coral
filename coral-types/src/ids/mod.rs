/// Define the block id table.
macro_rules! block_ids {
    ($($konst:ident = $id:literal, $name:literal;)+) => {
        $(pub const $konst: u8 = $id;)+

        pub const ALL: &[(u8, &str)] = &[$(($id, $name)),+];

        pub fn name_of(id: u8) -> Option<&'static str> {
            match id {
                $($id => Some($name),)+
                _ => None,
            }
        }

        pub fn from_name(name: &str) -> Option<u8> {
            match name.strip_prefix("minecraft:").unwrap_or(name) {
                $($name => Some($id),)+
                _ => None,
            }
        }
    };
}

/// Define the item id table.
macro_rules! item_ids {
    ($($konst:ident = $id:literal, $name:literal;)+) => {
        $(pub const $konst: i16 = $id;)+

        pub const ALL: &[(i16, &str)] = &[$(($id, $name)),+];

        pub fn name_of(id: i16) -> Option<&'static str> {
            match id {
                $($id => Some($name),)+
                _ => None,
            }
        }

        pub fn from_name(name: &str) -> Option<i16> {
            match name.strip_prefix("minecraft:").unwrap_or(name) {
                $($name => Some($id),)+
                _ => None,
            }
        }
    };
}

pub mod blocks;
pub mod items;

pub const EMPTY_SLOT: i16 = -1;

#[cfg(test)]
mod tests {
    use super::{blocks, items};

    #[test]
    fn no_id_appears_twice() {
        let mut block_ids: Vec<u8> = blocks::ALL.iter().map(|(id, _)| *id).collect();
        block_ids.sort_unstable();
        let total = block_ids.len();
        block_ids.dedup();
        assert_eq!(block_ids.len(), total, "a block id is listed twice");

        let mut item_ids: Vec<i16> = items::ALL.iter().map(|(id, _)| *id).collect();
        item_ids.sort_unstable();
        let total = item_ids.len();
        item_ids.dedup();
        assert_eq!(item_ids.len(), total, "an item id is listed twice");
    }

    #[test]
    fn no_name_appears_twice() {
        let mut names: Vec<&str> = blocks::ALL.iter().map(|(_, name)| *name).collect();
        names.sort_unstable();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "a block name is listed twice");

        let mut names: Vec<&str> = items::ALL.iter().map(|(_, name)| *name).collect();
        names.sort_unstable();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total, "an item name is listed twice");
    }

    #[test]
    fn the_lookups_round_trip_every_entry() {
        for (id, name) in blocks::ALL {
            assert_eq!(blocks::name_of(*id), Some(*name));
            assert_eq!(blocks::from_name(name), Some(*id));
        }
        for (id, name) in items::ALL {
            assert_eq!(items::name_of(*id), Some(*name));
            assert_eq!(items::from_name(name), Some(*id));
        }
    }

    #[test]
    fn a_minecraft_prefix_is_accepted() {
        assert_eq!(blocks::from_name("minecraft:stone"), Some(blocks::STONE));
        assert_eq!(items::from_name("minecraft:stick"), Some(items::STICK));
        assert_eq!(blocks::from_name("stone"), Some(blocks::STONE));
    }

    #[test]
    fn an_unknown_name_or_id_is_none() {
        assert_eq!(blocks::from_name("not_a_block"), None);
        assert_eq!(items::from_name(""), None);
        assert_eq!(items::name_of(30000), None);
    }

    #[test]
    fn air_is_zero() {
        assert_eq!(blocks::AIR, 0);
    }

    #[test]
    fn shared_ids_name_the_same_thing() {
        let mut shared = 0;
        for (id, name) in blocks::ALL {
            if let Some(item_name) = items::name_of(*id as i16) {
                assert_eq!(
                    item_name, *name,
                    "id {id} is \"{name}\" as a block but \"{item_name}\" as an item"
                );
                shared += 1;
            }
        }
        assert!(shared > 100, "only {shared} ids were shared, expected ~149");
    }

    #[test]
    fn the_fluid_ids_are_what_the_simulation_expects() {
        assert_eq!(blocks::FLOWING_WATER, 8);
        assert_eq!(blocks::WATER, 9);
        assert_eq!(blocks::FLOWING_LAVA, 10);
        assert_eq!(blocks::LAVA, 11);
    }
}
