use coral_protocol::packets::play::inventory::ItemStack;
use coral_world::nbt::NbtTag;

macro_rules! enchantments {
    ($($konst:ident = $id:literal, $name:literal, $display:literal, max $max:literal, weight $weight:literal, $cat:ident, cost ($mina:literal, $minb:literal)..($maxa:literal, $maxb:literal), excludes [$($excl:literal),*];)+) => {
        $(pub const $konst: i16 = $id;)+

        pub const ALL: &[Enchantment] = &[
            $(Enchantment {
                id: $id,
                name: $name,
                display_name: $display,
                max_level: $max,
                weight: $weight,
                category: Category::$cat,
                excludes: &[$($excl),*],
                min_cost: ($mina, $minb),
                max_cost: ($maxa, $maxb)
            }),+
        ];
    };
}

const ENCH_KEY: &str = "ench";
const TAG_COMPOUND: u8 = 10;

enchantments! {
    PROTECTION             =   0, "protection", "Protection", max 4, weight 10, Armor, cost (11, -10)..(11, 1), excludes [3, 1, 4];
    FIRE_PROTECTION        =   1, "fire_protection", "Fire Protection", max 4, weight 5, Armor, cost (8, 2)..(8, 10), excludes [3, 0, 4];
    FEATHER_FALLING        =   2, "feather_falling", "Feather Falling", max 4, weight 5, Boots, cost (6, -1)..(6, 5), excludes [];
    BLAST_PROTECTION       =   3, "blast_protection", "Blast Protection", max 4, weight 2, Armor, cost (8, -3)..(8, 5), excludes [1, 0, 4];
    PROJECTILE_PROTECTION  =   4, "projectile_protection", "Projectile Protection", max 4, weight 5, Armor, cost (6, -3)..(6, 3), excludes [0, 3, 1];
    RESPIRATION            =   5, "respiration", "Respiration", max 3, weight 2, Helmet, cost (10, 0)..(10, 30), excludes [];
    AQUA_AFFINITY          =   6, "aqua_affinity", "Aqua Affinity", max 1, weight 2, Helmet, cost (0, 1)..(0, 41), excludes [];
    THORNS                 =   7, "thorns", "Thorns", max 3, weight 1, Chestplate, cost (20, -10)..(10, 51), excludes [];
    DEPTH_STRIDER          =   8, "depth_strider", "Depth Strider", max 3, weight 2, Boots, cost (10, 0)..(10, 15), excludes [];
    SHARPNESS              =  16, "sharpness", "Sharpness", max 5, weight 10, Weapon, cost (11, -10)..(11, 10), excludes [17, 18];
    SMITE                  =  17, "smite", "Smite", max 5, weight 5, Weapon, cost (8, -3)..(8, 17), excludes [16, 18];
    BANE_OF_ARTHROPODS     =  18, "bane_of_arthropods", "Bane of Arthropods", max 5, weight 5, Weapon, cost (8, -3)..(8, 17), excludes [17, 16];
    KNOCKBACK              =  19, "knockback", "Knockback", max 2, weight 5, Weapon, cost (20, -15)..(10, 51), excludes [];
    FIRE_ASPECT            =  20, "fire_aspect", "Fire Aspect", max 2, weight 2, Weapon, cost (20, -10)..(10, 51), excludes [];
    LOOTING                =  21, "looting", "Looting", max 3, weight 2, Weapon, cost (9, 6)..(10, 51), excludes [];
    EFFICIENCY             =  32, "efficiency", "Efficiency", max 5, weight 10, Tool, cost (10, -9)..(10, 51), excludes [];
    SILK_TOUCH             =  33, "silk_touch", "Silk Touch", max 1, weight 1, Tool, cost (0, 15)..(10, 51), excludes [35];
    UNBREAKING             =  34, "unbreaking", "Unbreaking", max 3, weight 5, Breakable, cost (8, -3)..(10, 51), excludes [];
    FORTUNE                =  35, "fortune", "Fortune", max 3, weight 2, Tool, cost (9, 6)..(10, 51), excludes [33];
    POWER                  =  48, "power", "Power", max 5, weight 10, Bow, cost (10, -9)..(10, 6), excludes [];
    PUNCH                  =  49, "punch", "Punch", max 2, weight 2, Bow, cost (20, -8)..(20, 17), excludes [];
    FLAME                  =  50, "flame", "Flame", max 1, weight 2, Bow, cost (0, 20)..(0, 50), excludes [];
    INFINITY               =  51, "infinity", "Infinity", max 1, weight 1, Bow, cost (0, 20)..(0, 50), excludes [];
    LUCK_OF_THE_SEA        =  61, "luck_of_the_sea", "Luck of the Sea", max 3, weight 2, FishingRod, cost (9, 6)..(10, 51), excludes [];
    LURE                   =  62, "lure", "Lure", max 3, weight 2, FishingRod, cost (9, 6)..(10, 51), excludes [];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Armor,
    Helmet,
    Chestplate,
    Leggings,
    Boots,
    Weapon,
    Tool,
    Breakable,
    Bow,
    FishingRod,
}

#[derive(Debug, Clone, Copy)]
pub struct Enchantment {
    pub id: i16,
    pub name: &'static str,
    pub display_name: &'static str,
    pub max_level: u8,
    pub weight: u8,
    pub category: Category,
    pub excludes: &'static [i16],
    min_cost: (i32, i32),
    max_cost: (i32, i32),
}
impl Enchantment {
    /// The cheapest enchantment level that can yield this level
    pub fn min_cost(&self, level: u8) -> i32 {
        self.min_cost.0 * level as i32 + self.min_cost.1
    }

    pub fn max_cost(&self, level: u8) -> i32 {
        self.max_cost.0 * level as i32 + self.max_cost.1
    }

    /// Whether `other` may sit alongside this on one item
    pub fn conflicts_with(&self, other: i16) -> bool {
        other != self.id && self.excludes.contains(&other)
    }

    /// Clamp a level to what this enchantment supports
    /// Level 0 means absent
    pub fn clamp_level(&self, level: u8) -> u8 {
        level.min(self.max_level)
    }
}

pub fn by_id(id: i16) -> Option<&'static Enchantment> {
    ALL.iter().find(|e| e.id == id)
}
pub fn by_name(name: &str) -> Option<&'static Enchantment> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    ALL.iter().find(|e| e.name == name)
}

pub fn enchantments_of(stack: &ItemStack) -> Vec<(i16, u8)> {
    let Some(tags) = stack.nbt.as_ref() else {
        return Vec::new();
    };
    let Some(NbtTag::List(_, entries)) = tags.get(ENCH_KEY) else {
        return Vec::new();
    };

    let mut found = Vec::new();
    for entry in entries {
        let Some(id) = entry.get("id").and_then(NbtTag::as_i16_val) else {
            continue;
        };
        let level = entry.get("lvl").and_then(NbtTag::as_i16_val).unwrap_or(0);
        if level > 0 && by_id(id).is_some() {
            found.push((id, level.clamp(0, u8::MAX as i16) as u8));
        }
    }
    found
}

pub fn level_of(stack: &ItemStack, enchantment: i16) -> u8 {
    enchantments_of(stack)
        .into_iter()
        .find(|(id, _)| *id == enchantment)
        .map(|(_, level)| level)
        .unwrap_or(0)
}

pub fn is_enchanted(stack: &ItemStack) -> bool {
    !enchantments_of(stack).is_empty()
}

pub fn set_enchantments(stack: &mut ItemStack, enchantments: &[(i16, u8)]) {
    let mut tags = match stack.nbt.take() {
        Some(tags @ NbtTag::Compound(_)) => tags,
        _ => NbtTag::Compound(Vec::new()),
    };

    let kept: Vec<NbtTag> = enchantments
        .iter()
        .filter_map(|(id, level)| {
            let entry = by_id(*id)?;
            let level = entry.clamp_level(*level);
            if level == 0 {
                return None;
            }
            Some(NbtTag::Compound(vec![
                ("id".to_string(), NbtTag::Short(*id)),
                ("lvl".to_string(), NbtTag::Short(level as i16)),
            ]))
        })
        .collect();

    if kept.is_empty() {
        tags.remove(ENCH_KEY);
    } else {
        tags.set(ENCH_KEY, NbtTag::List(TAG_COMPOUND, kept));
    }

    stack.nbt = if tags.is_empty_compound() {
        None
    } else {
        Some(tags)
    };
}

pub fn add_enchantment(stack: &mut ItemStack, enchantment: i16, level: u8) -> bool {
    let Some(entry) = by_id(enchantment) else {
        return false;
    };
    let level = entry.clamp_level(level);
    if level == 0 {
        return false;
    }

    let mut current = enchantments_of(stack);
    if current.iter().any(|(id, _)| entry.conflicts_with(*id)) {
        return false;
    }

    match current.iter_mut().find(|(id, _)| *id == enchantment) {
        Some(existing) => existing.1 = existing.1.max(level),
        None => current.push((enchantment, level)),
    }
    set_enchantments(stack, &current);
    true
}

pub fn remove_enchantment(stack: &mut ItemStack, enchantment: i16) {
    let remaining: Vec<(i16, u8)> = enchantments_of(stack)
        .into_iter()
        .filter(|(id, _)| *id != enchantment)
        .collect();
    set_enchantments(stack, &remaining);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare(item_id: i16) -> ItemStack {
        ItemStack {
            item_id,
            count: 1,
            metadata: 0,
            durability: 0,
            nbt: None,
        }
    }

    #[test]
    fn the_registry_matches_the_published_1_8_table() {
        assert_eq!(ALL.len(), 25);
        assert_eq!(by_id(SHARPNESS).unwrap().max_level, 5);
        assert_eq!(by_id(SILK_TOUCH).unwrap().max_level, 1);
        assert_eq!(by_name("fortune").unwrap().id, FORTUNE);
        assert_eq!(by_name("minecraft:lure").unwrap().id, LURE);
        assert!(by_id(9).is_none(), "9 is a gap in the 1.8 numbering");
        assert!(by_name("mending").is_none(), "mending is 1.9");
    }

    #[test]
    fn no_entry_is_shadowed() {
        let mut ids: Vec<i16> = ALL.iter().map(|e| e.id).collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total);

        let mut names: Vec<&str> = ALL.iter().map(|e| e.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total);
    }

    #[test]
    fn exclusions_are_symmetric() {
        for entry in ALL {
            for &other in entry.excludes {
                let other = by_id(other)
                    .unwrap_or_else(|| panic!("{} excludes unknown id {other}", entry.name));
                assert!(
                    other.excludes.contains(&entry.id),
                    "{} excludes {} but not the reverse",
                    entry.name,
                    other.name
                );
            }
        }
    }

    #[test]
    fn nothing_excludes_itself() {
        for entry in ALL {
            assert!(
                !entry.conflicts_with(entry.id),
                "{} conflicts with itself",
                entry.name
            );
        }
    }

    #[test]
    fn every_entry_is_reachable() {
        for entry in ALL {
            assert!(entry.weight > 0, "{} has no weight", entry.name);
            assert!(entry.max_level > 0, "{} has no levels", entry.name);
            assert!(!entry.display_name.is_empty());
        }
    }

    #[test]
    fn cost_bands_are_ordered_at_every_level() {
        for entry in ALL {
            for level in 1..=entry.max_level {
                assert!(
                    entry.min_cost(level) <= entry.max_cost(level),
                    "{} level {level}: {}..{} runs backwards",
                    entry.name,
                    entry.min_cost(level),
                    entry.max_cost(level)
                );
            }
        }
    }

    #[test]
    fn a_bare_item_has_no_enchantments() {
        let stack = bare(276);
        assert!(!is_enchanted(&stack));
        assert_eq!(enchantments_of(&stack), vec![]);
        assert_eq!(level_of(&stack, SHARPNESS), 0);
    }

    #[test]
    fn an_enchantment_round_trips_through_nbt() {
        let mut stack = bare(276);
        assert!(add_enchantment(&mut stack, SHARPNESS, 3));

        assert!(stack.nbt.is_some(), "the tags were created");
        assert_eq!(level_of(&stack, SHARPNESS), 3);
        assert_eq!(enchantments_of(&stack), vec![(SHARPNESS, 3)]);
        assert!(is_enchanted(&stack));
    }

    #[test]
    fn the_tags_survive_a_wire_round_trip() {
        use coral_types::nbt::{NbtReader, NbtTag};

        let mut stack = bare(278);
        add_enchantment(&mut stack, EFFICIENCY, 4);
        add_enchantment(&mut stack, UNBREAKING, 2);

        let mut bytes = Vec::new();
        NbtTag::write_named_root("", stack.nbt.as_ref().unwrap(), &mut bytes);

        let mut reader = NbtReader::new(&bytes);
        let (_, tag) = reader.read_named_root();
        assert!(reader.is_ok(), "the tags did not parse back");

        let mut decoded = bare(278);
        decoded.nbt = Some(tag);

        let mut expected = enchantments_of(&stack);
        let mut got = enchantments_of(&decoded);
        expected.sort_unstable();
        got.sort_unstable();
        assert_eq!(got, expected);
    }

    #[test]
    fn a_level_above_the_maximum_is_clamped() {
        let mut stack = bare(276);
        add_enchantment(&mut stack, SHARPNESS, 99);
        assert_eq!(level_of(&stack, SHARPNESS), 5);

        let mut book = bare(403);
        add_enchantment(&mut book, SILK_TOUCH, 7);
        assert_eq!(level_of(&book, SILK_TOUCH), 1);
    }

    #[test]
    fn level_zero_and_unknown_ids_are_refused() {
        let mut stack = bare(276);
        assert!(!add_enchantment(&mut stack, SHARPNESS, 0));
        assert!(!add_enchantment(&mut stack, 9999, 1));
        assert!(!is_enchanted(&stack));
        assert!(stack.nbt.is_none(), "nothing was written");
    }

    #[test]
    fn a_conflicting_enchantment_is_refused_either_way_round() {
        let mut first = bare(278);
        assert!(add_enchantment(&mut first, SILK_TOUCH, 1));
        assert!(
            !add_enchantment(&mut first, FORTUNE, 3),
            "fortune must not join silk touch"
        );
        assert_eq!(enchantments_of(&first), vec![(SILK_TOUCH, 1)]);

        let mut other = bare(278);
        assert!(add_enchantment(&mut other, FORTUNE, 3));
        assert!(!add_enchantment(&mut other, SILK_TOUCH, 1));
        assert_eq!(enchantments_of(&other), vec![(FORTUNE, 3)]);
    }

    #[test]
    fn reapplying_keeps_the_higher_level() {
        let mut stack = bare(276);
        add_enchantment(&mut stack, SHARPNESS, 4);
        add_enchantment(&mut stack, SHARPNESS, 2);
        assert_eq!(level_of(&stack, SHARPNESS), 4);

        add_enchantment(&mut stack, SHARPNESS, 5);
        assert_eq!(level_of(&stack, SHARPNESS), 5);
        assert_eq!(enchantments_of(&stack).len(), 1, "still one entry");
    }

    #[test]
    fn several_compatible_enchantments_coexist() {
        let mut stack = bare(276);
        assert!(add_enchantment(&mut stack, SHARPNESS, 5));
        assert!(add_enchantment(&mut stack, KNOCKBACK, 2));
        assert!(add_enchantment(&mut stack, FIRE_ASPECT, 1));
        assert!(add_enchantment(&mut stack, UNBREAKING, 3));

        let mut found = enchantments_of(&stack);
        found.sort_unstable();
        let mut expected = vec![
            (SHARPNESS, 5),
            (KNOCKBACK, 2),
            (FIRE_ASPECT, 1),
            (UNBREAKING, 3),
        ];
        expected.sort_unstable();
        assert_eq!(found, expected);
    }

    #[test]
    fn removing_the_last_enchantment_clears_the_tags() {
        let mut stack = bare(276);
        add_enchantment(&mut stack, SHARPNESS, 3);
        remove_enchantment(&mut stack, SHARPNESS);

        assert!(!is_enchanted(&stack));
        assert!(
            stack.nbt.is_none(),
            "an empty ench list would still show as enchanted"
        );
    }

    #[test]
    fn removing_one_leaves_the_others() {
        let mut stack = bare(276);
        add_enchantment(&mut stack, SHARPNESS, 3);
        add_enchantment(&mut stack, KNOCKBACK, 1);

        remove_enchantment(&mut stack, SHARPNESS);
        assert_eq!(enchantments_of(&stack), vec![(KNOCKBACK, 1)]);
    }

    #[test]
    fn other_tags_are_left_alone() {
        use coral_types::nbt::NbtTag;

        let mut stack = bare(276);
        stack.nbt = Some(NbtTag::Compound(vec![(
            "display".to_string(),
            NbtTag::Compound(vec![("Name".to_string(), NbtTag::String("Sting".into()))]),
        )]));

        add_enchantment(&mut stack, SHARPNESS, 2);
        remove_enchantment(&mut stack, SHARPNESS);

        let tags = stack.nbt.as_ref().expect("the name survived");
        assert!(tags.get("display").is_some(), "display was dropped");
        assert!(tags.get(ENCH_KEY).is_none(), "ench should be gone");
    }

    #[test]
    fn a_zero_level_entry_is_not_an_enchantment() {
        use coral_types::nbt::NbtTag;

        let mut stack = bare(276);
        stack.nbt = Some(NbtTag::Compound(vec![(
            ENCH_KEY.to_string(),
            NbtTag::List(
                TAG_COMPOUND,
                vec![NbtTag::Compound(vec![
                    ("id".to_string(), NbtTag::Short(SHARPNESS)),
                    ("lvl".to_string(), NbtTag::Short(0)),
                ])],
            ),
        )]));

        assert!(!is_enchanted(&stack));
        assert_eq!(level_of(&stack, SHARPNESS), 0);
    }
}
