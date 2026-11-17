use coral_protocol::packets::play::inventory::ItemStack;

use crate::crafting::{Ingredient, Recipe};
use coral_types::ids::items;

fn stack(item_id: i16, count: u8, metadata: i16) -> ItemStack {
    ItemStack {
        item_id,
        count,
        metadata,
        durability: 0,
    }
}

/// Build shaped recipe from rows of key characters
fn shaped(rows: &[&str], key: &[(char, Ingredient)], result: ItemStack) -> Recipe {
    let height = rows.len();
    let width = rows[0].chars().count();

    assert!(
        (1..=3).contains(&width) && (1..=3).contains(&height),
        "recipe for {} is {width}x{height}",
        result.item_id
    );

    let mut pattern = Vec::with_capacity(width * height);
    for row in rows {
        assert_eq!(
            row.chars().count(),
            width,
            "recipe for {} has ragged rows",
            result.item_id
        );
        for symbol in row.chars() {
            if symbol == ' ' {
                pattern.push(None);
                continue;
            }
            let ingredient = key
                .iter()
                .find(|(k, _)| *k == symbol)
                .unwrap_or_else(|| {
                    panic!(
                        "recipe for {} uses '{symbol}', not in its key",
                        result.item_id
                    )
                })
                .1;
            pattern.push(Some(ingredient));
        }
    }

    Recipe::Shaped {
        width,
        height,
        pattern,
        result,
    }
}

fn shapeless(ingredients: &[Ingredient], result: ItemStack) -> Recipe {
    assert!(
        (1..=9).contains(&ingredients.len()),
        "recipe for {} has {} ingredients",
        result.item_id,
        ingredients.len()
    );
    Recipe::Shapeless {
        ingredients: ingredients.to_vec(),
        result,
    }
}

/// The five tool patterns
fn tool_set(material: Ingredient, results: (i16, i16, i16, i16, i16)) -> Vec<Recipe> {
    let (sword, shovel, pickaxe, axe, hoe) = results;
    let stick = Ingredient::any(items::STICK);
    let key: &[(char, Ingredient)] = &[('X', material), ('S', stick)];

    vec![
        shaped(&["X", "X", "S"], key, stack(sword, 1, 0)),
        shaped(&["X", "S", "S"], key, stack(shovel, 1, 0)),
        shaped(&["XXX", " S ", " S "], key, stack(pickaxe, 1, 0)),
        shaped(&["XX", "XS", " S"], key, stack(axe, 1, 0)),
        shaped(&["XX", " S", " S"], key, stack(hoe, 1, 0)),
    ]
}

/// The four armor patterns
fn armor_set(material: Ingredient, results: (i16, i16, i16, i16)) -> Vec<Recipe> {
    let (helmet, chestplate, leggings, boots) = results;
    let key: &[(char, Ingredient)] = &[('X', material)];

    vec![
        shaped(&["XXX", "X X"], key, stack(helmet, 1, 0)),
        shaped(&["X X", "XXX", "XXX"], key, stack(chestplate, 1, 0)),
        shaped(&["XXX", "X X", "X X"], key, stack(leggings, 1, 0)),
        shaped(&["X X", "X X"], key, stack(boots, 1, 0)),
    ]
}

fn packing(item: Ingredient, block_id: i16, block_meta: i16) -> Vec<Recipe> {
    vec![
        shaped(
            &["XXX", "XXX", "XXX"],
            &[('X', item)],
            stack(block_id, 1, block_meta),
        ),
        shapeless(
            &[Ingredient::exact(block_id, block_meta)],
            stack(item.item_id, 9, item.metadata.unwrap_or(0)),
        ),
    ]
}

pub fn all() -> Vec<Recipe> {
    let planks = Ingredient::any(items::PLANKS);
    let stick = Ingredient::any(items::STICK);
    let cobble = Ingredient::any(items::COBBLESTONE);
    let iron = Ingredient::any(items::IRON_INGOT);
    let gold = Ingredient::any(items::GOLD_INGOT);
    let diamond = Ingredient::any(items::DIAMOND);

    let mut recipes = Vec::new();

    for wood in 0..4 {
        recipes.push(shapeless(
            &[Ingredient::exact(items::LOG, wood)],
            stack(items::PLANKS, 4, wood),
        ));
    }
    for wood in 0..2 {
        recipes.push(shapeless(
            &[Ingredient::exact(items::LOG2, wood)],
            stack(items::PLANKS, 4, wood + 4),
        ));
    }

    recipes.push(shaped(
        &["X", "X"],
        &[('X', planks)],
        stack(items::STICK, 4, 0),
    ));
    recipes.push(shaped(
        &["XX", "XX"],
        &[('X', planks)],
        stack(items::CRAFTING_TABLE, 1, 0),
    ));
    recipes.push(shaped(
        &["XXX", "X X", "XXX"],
        &[('X', planks)],
        stack(items::CHEST, 1, 0),
    ));
    recipes.push(shaped(
        &["XXX", "X X", "XXX"],
        &[('X', cobble)],
        stack(items::FURNACE, 1, 0),
    ));
    recipes.push(shaped(
        &["C", "S"],
        &[('C', Ingredient::any(items::COAL)), ('S', stick)],
        stack(items::TORCH, 4, 0),
    ));

    recipes.extend(tool_set(
        planks,
        (
            items::WOODEN_SWORD,
            items::WOODEN_SHOVEL,
            items::WOODEN_PICKAXE,
            items::WOODEN_AXE,
            items::WOODEN_HOE,
        ),
    ));
    recipes.extend(tool_set(
        cobble,
        (
            items::STONE_SWORD,
            items::STONE_SHOVEL,
            items::STONE_PICKAXE,
            items::STONE_AXE,
            items::STONE_HOE,
        ),
    ));
    recipes.extend(tool_set(
        iron,
        (
            items::IRON_SWORD,
            items::IRON_SHOVEL,
            items::IRON_PICKAXE,
            items::IRON_AXE,
            items::IRON_HOE,
        ),
    ));
    recipes.extend(tool_set(
        gold,
        (
            items::GOLDEN_SWORD,
            items::GOLDEN_SHOVEL,
            items::GOLDEN_PICKAXE,
            items::GOLDEN_AXE,
            items::GOLDEN_HOE,
        ),
    ));
    recipes.extend(tool_set(
        diamond,
        (
            items::DIAMOND_SWORD,
            items::DIAMOND_SHOVEL,
            items::DIAMOND_PICKAXE,
            items::DIAMOND_AXE,
            items::DIAMOND_HOE,
        ),
    ));

    recipes.extend(armor_set(
        Ingredient::any(items::LEATHER),
        (
            items::LEATHER_HELMET,
            items::LEATHER_CHESTPLATE,
            items::LEATHER_LEGGINGS,
            items::LEATHER_BOOTS,
        ),
    ));
    recipes.extend(armor_set(
        iron,
        (
            items::IRON_HELMET,
            items::IRON_CHESTPLATE,
            items::IRON_LEGGINGS,
            items::IRON_BOOTS,
        ),
    ));
    recipes.extend(armor_set(
        gold,
        (
            items::GOLDEN_HELMET,
            items::GOLDEN_CHESTPLATE,
            items::GOLDEN_LEGGINGS,
            items::GOLDEN_BOOTS,
        ),
    ));
    recipes.extend(armor_set(
        diamond,
        (
            items::DIAMOND_HELMET,
            items::DIAMOND_CHESTPLATE,
            items::DIAMOND_LEGGINGS,
            items::DIAMOND_BOOTS,
        ),
    ));

    recipes.extend(packing(iron, items::IRON_BLOCK, 0));
    recipes.extend(packing(gold, items::GOLD_BLOCK, 0));
    recipes.extend(packing(diamond, items::DIAMOND_BLOCK, 0));
    recipes.extend(packing(Ingredient::any(items::COAL), items::COAL_BLOCK, 0));
    recipes.extend(packing(
        Ingredient::any(items::EMERALD),
        items::EMERALD_BLOCK,
        0,
    ));
    recipes.extend(packing(
        Ingredient::any(items::REDSTONE),
        items::REDSTONE_BLOCK,
        0,
    ));
    recipes.extend(packing(
        Ingredient::exact(items::DYE, items::COBBLESTONE),
        items::LAPIS_BLOCK,
        0,
    ));
    recipes.extend(packing(
        Ingredient::any(items::GOLD_NUGGET),
        items::GOLD_INGOT,
        0,
    ));

    recipes.push(shaped(
        &["XX", "XX"],
        &[('X', Ingredient::any(items::STONE))],
        stack(items::STONEBRICK, 4, 0),
    ));
    recipes.push(shaped(
        &["XX", "XX"],
        &[('X', Ingredient::any(items::SAND))],
        stack(items::SANDSTONE, 1, 0),
    ));
    recipes.push(shaped(
        &["XX", "XX"],
        &[('X', Ingredient::any(items::BRICK))],
        stack(items::BRICK_BLOCK, 1, 0),
    ));
    recipes.push(shaped(
        &["XX", "XX"],
        &[('X', Ingredient::any(items::CLAY_BALL))],
        stack(items::CLAY, 1, 0),
    ));
    recipes.push(shaped(
        &["XX", "XX"],
        &[('X', Ingredient::any(items::SNOWBALL))],
        stack(items::SNOW, 1, 0),
    ));
    recipes.push(shaped(
        &["XX", "XX"],
        &[('X', Ingredient::any(items::GLOWSTONE_DUST))],
        stack(items::GLOWSTONE, 1, 0),
    ));
    recipes.push(shaped(
        &["XX", "XX"],
        &[('X', Ingredient::any(items::STRING))],
        stack(items::WOOL, 1, 0),
    ));

    recipes.push(shaped(
        &["XXX"],
        &[('X', Ingredient::any(items::STONE))],
        stack(items::STONE_SLAB, 6, 0),
    ));
    recipes.push(shaped(
        &["XXX"],
        &[('X', cobble)],
        stack(items::STONE_SLAB, 6, 3),
    ));
    recipes.push(shaped(
        &["X  ", "XX ", "XXX"],
        &[('X', planks)],
        stack(items::OAK_STAIRS, 4, 0),
    ));
    recipes.push(shaped(
        &["X  ", "XX ", "XXX"],
        &[('X', cobble)],
        stack(items::STONE_STAIRS, 4, 0),
    ));

    recipes.push(shaped(
        &["XXX", "XXX"],
        &[('X', Ingredient::any(items::GLASS))],
        stack(items::GLASS_PANE, 16, 0),
    ));
    recipes.push(shaped(
        &["XXX", "XXX"],
        &[('X', iron)],
        stack(items::IRON_BARS, 16, 0),
    ));
    recipes.push(shaped(
        &["XXX", "XXX", " S "],
        &[('X', planks), ('S', stick)],
        stack(items::SIGN, 3, 0),
    ));
    recipes.push(shaped(
        &["X X", "XXX", "X X"],
        &[('X', stick)],
        stack(items::LADDER, 3, 0),
    ));
    recipes.push(shaped(
        &["PPP", "BBB", "PPP"],
        &[('P', planks), ('B', Ingredient::any(items::BOOK))],
        stack(items::BOOKSHELF, 1, 0),
    ));
    recipes.push(shaped(
        &["GSG", "SGS", "GSG"],
        &[
            ('G', Ingredient::any(items::GUNPOWDER)),
            ('S', Ingredient::any(items::SAND)),
        ],
        stack(items::TNT, 1, 0),
    ));

    recipes.push(shaped(
        &["X X", " X "],
        &[('X', iron)],
        stack(items::BUCKET, 1, 0),
    ));
    recipes.push(shaped(
        &[" X", "X "],
        &[('X', iron)],
        stack(items::SHEARS, 1, 0),
    ));
    recipes.push(shapeless(
        &[iron, Ingredient::any(items::FLINT)],
        stack(items::FLINT_AND_STEEL, 1, 0),
    ));
    recipes.push(shaped(
        &["X X", " X "],
        &[('X', planks)],
        stack(items::BOWL, 4, 0),
    ));
    recipes.push(shaped(
        &[" XS", "X S", " XS"],
        &[('X', stick), ('S', Ingredient::any(items::STRING))],
        stack(items::BOW, 1, 0),
    ));
    recipes.push(shaped(
        &["  X", " XS", "X S"],
        &[('X', stick), ('S', Ingredient::any(items::STRING))],
        stack(items::FISHING_ROD, 1, 0),
    ));
    recipes.push(shaped(
        &["F", "S", "T"],
        &[
            ('F', Ingredient::any(items::FLINT)),
            ('S', stick),
            ('T', Ingredient::any(items::FEATHER)),
        ],
        stack(items::ARROW, 4, 0),
    ));
    recipes.push(shaped(
        &["WWW", "PPP"],
        &[('W', Ingredient::any(items::WOOL)), ('P', planks)],
        stack(items::BED, 1, 0),
    ));

    recipes.push(shaped(
        &["XXX"],
        &[('X', Ingredient::any(items::WHEAT))],
        stack(items::BREAD, 1, 0),
    ));
    recipes.push(shaped(
        &["XXX"],
        &[('X', Ingredient::any(items::REEDS))],
        stack(items::PAPER, 3, 0),
    ));
    recipes.push(shapeless(
        &[Ingredient::any(items::REEDS)],
        stack(items::SUGAR, 1, 0),
    ));
    recipes.push(shapeless(
        &[
            Ingredient::any(items::PAPER),
            Ingredient::any(items::PAPER),
            Ingredient::any(items::PAPER),
            Ingredient::any(items::LEATHER),
        ],
        stack(items::BOOK, 1, 0),
    ));
    recipes.push(shapeless(
        &[
            Ingredient::any(items::BOWL),
            Ingredient::any(items::BROWN_MUSHROOM),
            Ingredient::any(items::RED_MUSHROOM),
        ],
        stack(items::MUSHROOM_STEW, 1, 0),
    ));
    recipes.push(shaped(
        &["WCW"],
        &[
            ('W', Ingredient::any(items::WHEAT)),
            ('C', Ingredient::exact(items::DYE, items::DIRT)),
        ],
        stack(items::COOKIE, 8, 0),
    ));
    recipes.push(shaped(
        &["GGG", "GAG", "GGG"],
        &[('G', gold), ('A', Ingredient::any(items::APPLE))],
        stack(items::GOLDEN_APPLE, 1, 0),
    ));
    recipes.push(shaped(
        &["MMM", "SES", "WWW"],
        &[
            ('M', Ingredient::any(items::MILK_BUCKET)),
            ('S', Ingredient::any(items::SUGAR)),
            ('E', Ingredient::any(items::EGG)),
            ('W', Ingredient::any(items::WHEAT)),
        ],
        stack(items::CAKE, 1, 0),
    ));

    recipes
}

#[cfg(test)]
mod tests {
    use super::super::{CraftingGrid, RecipeRegistry};
    use super::*;

    fn stack_opt(item_id: i16, metadata: i16) -> Option<ItemStack> {
        Some(stack(item_id, 1, metadata))
    }

    fn grid(rows: &[&str], key: &[(char, (i16, i16))]) -> CraftingGrid {
        let mut grid = CraftingGrid::bench();
        for (y, row) in rows.iter().enumerate() {
            for (x, symbol) in row.chars().enumerate() {
                if symbol == ' ' {
                    continue;
                }
                let (id, meta) = key.iter().find(|(k, _)| *k == symbol).unwrap().1;
                grid.set(y * 3 + x, stack_opt(id, meta));
            }
        }
        grid
    }

    fn small_grid(rows: &[&str], key: &[(char, (i16, i16))]) -> CraftingGrid {
        let mut grid = CraftingGrid::player();
        for (y, row) in rows.iter().enumerate() {
            for (x, symbol) in row.chars().enumerate() {
                if symbol == ' ' {
                    continue;
                }
                let (id, meta) = key.iter().find(|(k, _)| *k == symbol).unwrap().1;
                grid.set(y * 2 + x, stack_opt(id, meta));
            }
        }
        grid
    }

    #[test]
    fn the_table_builds() {
        let recipes = all();
        assert!(recipes.len() > 60, "only {} recipes", recipes.len());
    }

    #[test]
    fn a_log_makes_four_planks_of_its_own_wood() {
        let registry = RecipeRegistry::vanilla();

        for wood in 0..4i16 {
            let mut g = CraftingGrid::player();
            g.set(0, stack_opt(items::LOG, wood));
            let result = registry.result_for(&g).expect("log {wood} makes planks");
            assert_eq!(
                (result.item_id, result.count, result.metadata),
                (items::PLANKS, 4, wood)
            );
        }
        for wood in 0..2i16 {
            let mut g = CraftingGrid::player();
            g.set(0, stack_opt(items::LOG2, wood));
            let result = registry.result_for(&g).expect("log2 makes planks");
            assert_eq!(
                (result.item_id, result.count, result.metadata),
                (items::PLANKS, 4, wood + 4)
            );
        }
    }

    #[test]
    fn the_first_few_minutes_work() {
        let registry = RecipeRegistry::vanilla();
        let key = &[
            ('P', (items::PLANKS, 0)),
            ('S', (items::STICK, 0)),
            ('C', (items::COBBLESTONE, 0)),
            ('L', (items::COAL, 0)),
        ];

        let sticks = registry
            .result_for(&small_grid(&["P ", "P "], key))
            .expect("two planks make sticks");
        assert_eq!((sticks.item_id, sticks.count), (items::STICK, 4));

        let table = registry
            .result_for(&small_grid(&["PP", "PP"], key))
            .expect("four planks make a table");
        assert_eq!((table.item_id, table.count), (items::CRAFTING_TABLE, 1));

        let pick = registry
            .result_for(&grid(&["PPP", " S ", " S "], key))
            .expect("planks and sticks make a pickaxe");
        assert_eq!((pick.item_id, pick.count), (items::WOODEN_PICKAXE, 1));

        let stone_pick = registry
            .result_for(&grid(&["CCC", " S ", " S "], key))
            .expect("cobblestone makes a pickaxe");
        assert_eq!(stone_pick.item_id, items::STONE_PICKAXE);

        let furnace = registry
            .result_for(&grid(&["CCC", "C C", "CCC"], key))
            .expect("eight cobblestone make a furnace");
        assert_eq!((furnace.item_id, furnace.count), (items::FURNACE, 1));

        for coal_meta in [0, 1] {
            let mut g = CraftingGrid::bench();
            g.set(0, stack_opt(items::COAL, coal_meta));
            g.set(3, stack_opt(items::STICK, 0));
            let torch = registry
                .result_for(&g)
                .expect("coal over a stick makes torches");
            assert_eq!((torch.item_id, torch.count), (items::TORCH, 4));
        }
    }

    #[test]
    fn a_pickaxe_can_be_laid_out_either_handed() {
        let registry = RecipeRegistry::vanilla();
        let key = &[('P', (items::PLANKS, 0)), ('S', (items::STICK, 0))];

        let right = registry.result_for(&grid(&["PP ", "PS ", " S "], key));
        let left = registry.result_for(&grid(&[" PP", " SP", " S "], key));

        assert_eq!(right.as_ref().map(|s| s.item_id), Some(items::WOODEN_AXE));
        assert_eq!(
            left.as_ref().map(|s| s.item_id),
            Some(items::WOODEN_AXE),
            "the axe must craft mirrored as well"
        );
    }

    #[test]
    fn storage_blocks_pack_and_unpack() {
        let registry = RecipeRegistry::vanilla();

        for (item, item_meta, block) in [
            (items::IRON_INGOT, 0, items::IRON_BLOCK),
            (items::GOLD_INGOT, 0, items::GOLD_BLOCK),
            (items::DIAMOND, 0, items::DIAMOND_BLOCK),
            (items::COAL, 0, items::COAL_BLOCK),
            (items::EMERALD, 0, items::EMERALD_BLOCK),
            (items::REDSTONE, 0, items::REDSTONE_BLOCK),
            (items::DYE, items::COBBLESTONE, items::LAPIS_BLOCK),
            (items::GOLD_NUGGET, 0, items::GOLD_INGOT),
        ] {
            let mut packed = CraftingGrid::bench();
            for slot in 0..9 {
                packed.set(slot, stack_opt(item, item_meta));
            }
            let block_result = registry
                .result_for(&packed)
                .unwrap_or_else(|| panic!("nine of {item} makes nothing"));
            assert_eq!((block_result.item_id, block_result.count), (block, 1));

            let mut unpacked = CraftingGrid::player();
            unpacked.set(0, stack_opt(block, 0));
            let item_result = registry
                .result_for(&unpacked)
                .unwrap_or_else(|| panic!("block {block} unpacks to nothing"));
            assert_eq!(
                (item_result.item_id, item_result.count, item_result.metadata),
                (item, 9, item_meta),
                "block {block} must give back what made it"
            );
        }
    }

    #[test]
    fn a_full_grid_of_iron_is_a_block_not_a_chestplate() {
        let registry = RecipeRegistry::vanilla();

        let mut full = CraftingGrid::bench();
        for slot in 0..9 {
            full.set(slot, stack_opt(items::IRON_INGOT, 0));
        }
        assert_eq!(
            registry.result_for(&full).unwrap().item_id,
            items::IRON_BLOCK
        );

        let mut ring = CraftingGrid::bench();
        for slot in [0, 2, 3, 4, 5, 6, 7, 8] {
            ring.set(slot, stack_opt(items::IRON_INGOT, 0));
        }
        assert_eq!(
            registry.result_for(&ring).unwrap().item_id,
            items::IRON_CHESTPLATE,
            "a hole in the top middle makes it a chestplate"
        );
    }

    #[test]
    fn a_bed_takes_any_wool_colour() {
        let registry = RecipeRegistry::vanilla();

        for colour in [0i16, 11, 14] {
            let mut g = CraftingGrid::bench();
            for slot in 0..3 {
                g.set(slot, stack_opt(items::WOOL, colour));
            }
            for slot in 3..6 {
                g.set(slot, stack_opt(items::PLANKS, 0));
            }
            let bed = registry
                .result_for(&g)
                .unwrap_or_else(|| panic!("wool colour {colour} makes no bed"));
            assert_eq!((bed.item_id, bed.count), (items::BED, 1));
        }
    }

    #[test]
    fn cookies_need_cocoa_not_any_dye() {
        let registry = RecipeRegistry::vanilla();

        let mut cocoa = CraftingGrid::bench();
        cocoa.set(0, stack_opt(items::WHEAT, 0));
        cocoa.set(1, stack_opt(items::DYE, items::DIRT));
        cocoa.set(2, stack_opt(items::WHEAT, 0));
        assert_eq!(registry.result_for(&cocoa).unwrap().item_id, items::COOKIE);

        let mut lapis = CraftingGrid::bench();
        lapis.set(0, stack_opt(items::WHEAT, 0));
        lapis.set(1, stack_opt(items::DYE, items::COBBLESTONE));
        lapis.set(2, stack_opt(items::WHEAT, 0));
        assert!(
            registry.result_for(&lapis).is_none(),
            "lapis is not cocoa beans"
        );
    }

    #[test]
    fn slabs_carry_their_material_in_metadata() {
        let registry = RecipeRegistry::vanilla();

        let mut stone = CraftingGrid::bench();
        for slot in 0..3 {
            stone.set(slot, stack_opt(items::STONE, 0));
        }
        let stone_slab = registry.result_for(&stone).unwrap();
        assert_eq!(
            (stone_slab.item_id, stone_slab.count, stone_slab.metadata),
            (items::STONE_SLAB, 6, 0)
        );

        let mut cobble = CraftingGrid::bench();
        for slot in 0..3 {
            cobble.set(slot, stack_opt(items::COBBLESTONE, 0));
        }
        let cobble_slab = registry.result_for(&cobble).unwrap();
        assert_eq!(
            (cobble_slab.item_id, cobble_slab.count, cobble_slab.metadata),
            (items::STONE_SLAB, 6, 3)
        );
    }

    #[test]
    fn bench_recipes_need_a_bench() {
        let registry = RecipeRegistry::vanilla();

        let mut g = CraftingGrid::player();
        for slot in 0..4 {
            g.set(slot, stack_opt(items::IRON_INGOT, 0));
        }
        assert!(
            registry.result_for(&g).is_none(),
            "four iron in a 2x2 makes nothing"
        );
    }

    #[test]
    fn every_recipe_matches_the_grid_it_describes() {
        let registry = RecipeRegistry::vanilla();

        for recipe in registry.recipes() {
            let mut g = CraftingGrid::bench();

            match recipe {
                Recipe::Shaped {
                    width,
                    height,
                    pattern,
                    ..
                } => {
                    for y in 0..*height {
                        for x in 0..*width {
                            if let Some(ingredient) = pattern[y * width + x] {
                                g.set(
                                    y * 3 + x,
                                    stack_opt(ingredient.item_id, ingredient.metadata.unwrap_or(0)),
                                );
                            }
                        }
                    }
                }
                Recipe::Shapeless { ingredients, .. } => {
                    for (slot, ingredient) in ingredients.iter().enumerate() {
                        g.set(
                            slot,
                            stack_opt(ingredient.item_id, ingredient.metadata.unwrap_or(0)),
                        );
                    }
                }
            }

            let got = registry.result_for(&g).unwrap_or_else(|| {
                panic!("recipe making {} matches no grid", recipe.result().item_id)
            });
            assert_eq!(
                got.item_id,
                recipe.result().item_id,
                "recipe making {} got {} instead",
                recipe.result().item_id,
                got.item_id
            );
        }
    }
}
