use coral_protocol::packets::play::inventory::ItemStack;

pub mod recipes;

/// What recipe slot accepts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ingredient {
    pub item_id: i16,
    pub metadata: Option<i16>,
}
impl Ingredient {
    /// Any metadata
    pub const fn any(item_id: i16) -> Self {
        Self {
            item_id,
            metadata: None,
        }
    }

    /// One specific metadata value
    pub const fn exact(item_id: i16, metadata: i16) -> Self {
        Self {
            item_id,
            metadata: Some(metadata),
        }
    }

    pub fn accepts(&self, stack: &ItemStack) -> bool {
        self.item_id == stack.item_id
            && match self.metadata {
                Some(meta) => meta == stack.metadata,
                None => true,
            }
    }
}

/// The crafting slots of an open window:
/// 2x2 in the player's own inventory
/// 3x3 at a crafting table
#[derive(Debug, Clone)]
pub struct CraftingGrid {
    width: usize,
    height: usize,
    cells: Vec<Option<ItemStack>>,
}
impl CraftingGrid {
    /// A grid of `width` x `height` empty slots
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![None; width * height],
        }
    }

    pub fn player() -> Self {
        Self::new(2, 2)
    }

    pub fn bench() -> Self {
        Self::new(3, 3)
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(Option::is_none)
    }

    pub fn get(&self, index: usize) -> Option<&ItemStack> {
        self.cells.get(index).and_then(Option::as_ref)
    }

    pub fn set(&mut self, index: usize, stack: Option<ItemStack>) {
        if let Some(cell) = self.cells.get_mut(index) {
            *cell = stack;
        }
    }

    pub fn at(&self, x: usize, y: usize) -> Option<&ItemStack> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.get(y * self.width() + x)
    }

    pub fn cells(&self) -> &[Option<ItemStack>] {
        &self.cells
    }

    /// Take on item from every occupied slot, which is what crafting consumes regardless of how many a slot held
    ///
    /// Returns the stack that were emptied to nothing
    pub fn consume_one_of_each(&mut self) {
        for cell in self.cells.iter_mut() {
            let Some(stack) = cell else {
                continue;
            };

            if stack.count <= 1 {
                *cell = None;
            } else {
                stack.count -= 1;
            }
        }
    }

    /// How many times this grid could craft limited by its smallest stack
    ///
    /// Used by shift-clicking the output
    pub fn max_crafts(&self) -> u8 {
        self.cells
            .iter()
            .flatten()
            .map(|stack| stack.count)
            .min()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub enum Recipe {
    /// A pattern that must appear in the grid possiblty offset and possibly mirrored
    /// `pattern` is row-major and `None` is a slot that must be empty *within* the pattern's own bounds
    Shaped {
        width: usize,
        height: usize,
        pattern: Vec<Option<Ingredient>>,
        result: ItemStack,
    },
    /// Ingredients in any arrangement
    /// Every occupied cell must be accounted for and every ingredient must be present
    Shapeless {
        ingredients: Vec<Ingredient>,
        result: ItemStack,
    },
}
impl Recipe {
    pub fn result(&self) -> &ItemStack {
        match self {
            Self::Shaped { result, .. } | Self::Shapeless { result, .. } => result,
        }
    }

    /// The smallest grid this recipe can be made in
    pub fn min_grid(&self) -> (usize, usize) {
        match self {
            Self::Shaped { width, height, .. } => (*width, *height),
            Self::Shapeless { ingredients, .. } => {
                // Anything up to four fits a 2x2; more needs a bench
                if ingredients.len() <= 4 {
                    (2, 2)
                } else {
                    (3, 3)
                }
            }
        }
    }

    pub fn matches(&self, grid: &CraftingGrid) -> bool {
        match self {
            Self::Shaped {
                width,
                height,
                pattern,
                ..
            } => matches_shaped(grid, *width, *height, pattern),
            Self::Shapeless { ingredients, .. } => matches_shapeless(grid, ingredients),
        }
    }
}

/// Try the pattern at every offset it fits, both as written and mirrored
fn matches_shaped(
    grid: &CraftingGrid,
    width: usize,
    height: usize,
    pattern: &[Option<Ingredient>],
) -> bool {
    if width > grid.width() || height > grid.height() {
        return false;
    }

    for offset_y in 0..=(grid.height() - height) {
        for offset_x in 0..=(grid.width() - width) {
            for mirrored in [false, true] {
                if matches_at(grid, offset_x, offset_y, width, height, pattern, mirrored) {
                    return true;
                }
            }
        }
    }
    false
}

/// Check the pattern against the grid with its top-left corner at `(offset_x, offset_y)`
fn matches_at(
    grid: &CraftingGrid,
    offset_x: usize,
    offset_y: usize,
    width: usize,
    height: usize,
    pattern: &[Option<Ingredient>],
    mirrored: bool,
) -> bool {
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let in_pattern =
                x >= offset_x && x < offset_x + width && y >= offset_y && y < offset_y + height;

            let wanted = if in_pattern {
                let px = x - offset_x;
                let py = y - offset_y;
                let px = if mirrored { width - 1 - px } else { px };
                pattern[py * width + px]
            } else {
                None
            };

            match (wanted, grid.at(x, y)) {
                (Some(ingredient), Some(stack)) if ingredient.accepts(stack) => {}
                (None, None) => {}
                _ => return false,
            }
        }
    }
    true
}

fn matches_shapeless(grid: &CraftingGrid, ingredients: &[Ingredient]) -> bool {
    let occupied: Vec<&ItemStack> = grid.cells().iter().flatten().collect();

    if occupied.len() != ingredients.len() {
        return false;
    }

    let mut claimed = vec![false; occupied.len()];
    for ingredient in ingredients {
        let found = occupied
            .iter()
            .enumerate()
            .position(|(i, stack)| !claimed[i] && ingredient.accepts(stack));

        match found {
            Some(i) => claimed[i] = true,
            None => return false,
        }
    }
    true
}

pub struct RecipeRegistry {
    recipes: Vec<Recipe>,
}
impl RecipeRegistry {
    pub fn empty() -> Self {
        Self { recipes: vec![] }
    }

    pub fn vanilla() -> Self {
        Self {
            recipes: recipes::all(),
        }
    }

    pub fn add(&mut self, recipe: Recipe) {
        self.recipes.push(recipe);
    }

    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    pub fn recipes(&self) -> &[Recipe] {
        &self.recipes
    }

    /// What this grid makes if anything
    pub fn result_for(&self, grid: &CraftingGrid) -> Option<ItemStack> {
        if grid.is_empty() {
            return None;
        }
        self.recipes
            .iter()
            .find(|recipe| recipe.matches(grid))
            .map(|recipe| recipe.result().clone())
    }

    pub fn recipe_for(&self, grid: &CraftingGrid) -> Option<&Recipe> {
        if grid.is_empty() {
            return None;
        }
        self.recipes.iter().find(|recipe| recipe.matches(grid))
    }
}
impl Default for RecipeRegistry {
    fn default() -> Self {
        Self::vanilla()
    }
}

/// What a click on the output slot did
#[derive(Debug, Clone)]
pub enum OutputClick {
    /// The grid is untouched:
    /// Either no recupe matches or
    /// the cursor holds something the result will not stack onto
    Rejected,
    /// The grid was consumed and the cursor now holds this
    Crafted(ItemStack),
}

/// Whether `result` can be added  to what the cursor already holds
fn cursor_accepts(cursor: Option<&ItemStack>, result: &ItemStack, max_stack: u8) -> bool {
    match cursor {
        None => true,
        Some(held) => {
            held.item_id == result.item_id
                && held.metadata == result.metadata
                && held.durability == result.durability
                && held.count.saturating_add(result.count) <= max_stack
        }
    }
}

/// Take the output once as a left-click on the result slot does
pub fn craft_once(
    grid: &mut CraftingGrid,
    registry: &RecipeRegistry,
    cursor: Option<&ItemStack>,
    max_stack: u8,
) -> OutputClick {
    let Some(result) = registry.result_for(grid) else {
        return OutputClick::Rejected;
    };
    if !cursor_accepts(cursor, &result, max_stack) {
        return OutputClick::Rejected;
    }

    grid.consume_one_of_each();

    let new_cursor = match cursor {
        Some(held) => ItemStack {
            count: held.count + result.count,
            ..result
        },
        None => result,
    };
    OutputClick::Crafted(new_cursor)
}

#[cfg(test)]
mod output_tests {
    use super::*;
    use coral_types::ids::items::{PLANKS, STICK};

    fn stack(item_id: i16, count: u8) -> ItemStack {
        ItemStack {
            item_id,
            count,
            metadata: 0,
            durability: 0,
            nbt: None,
        }
    }

    #[track_caller]
    fn assert_crafted(got: &OutputClick, item_id: i16, count: u8) {
        match got {
            OutputClick::Crafted(cursor) => {
                assert_eq!(
                    (cursor.item_id, cursor.count),
                    (item_id, count),
                    "wrong cursor after crafting"
                );
            }
            OutputClick::Rejected => panic!("expected a craft, got Rejected"),
        }
    }

    #[track_caller]
    fn assert_rejected(got: &OutputClick) {
        if let OutputClick::Crafted(cursor) = got {
            panic!(
                "expected Rejected, but crafted {} x{}",
                cursor.item_id, cursor.count
            );
        }
    }

    fn sticks_grid() -> CraftingGrid {
        let mut grid = CraftingGrid::player();
        grid.set(0, Some(stack(PLANKS, 1)));
        grid.set(2, Some(stack(PLANKS, 1)));
        grid
    }

    fn registry() -> RecipeRegistry {
        RecipeRegistry::vanilla()
    }

    #[test]
    fn taking_the_output_consumes_the_grid() {
        let mut grid = sticks_grid();
        let got = craft_once(&mut grid, &registry(), None, 64);

        assert_crafted(&got, STICK, 4);
        assert!(grid.is_empty(), "both planks must be gone");
    }

    #[test]
    fn the_output_cannot_be_taken_twice_from_one_grid() {
        let mut grid = sticks_grid();
        let registry = registry();

        assert_crafted(&craft_once(&mut grid, &registry, None, 64), STICK, 4);
        assert_rejected(&craft_once(&mut grid, &registry, None, 64));
    }

    #[test]
    fn a_rejected_click_does_not_touch_the_grid() {
        let mut grid = sticks_grid();
        let holding = stack(999, 1);

        assert_rejected(&craft_once(&mut grid, &registry(), Some(&holding), 64));
        assert_eq!(grid.get(0).unwrap().count, 1, "the planks are still there");
        assert_eq!(grid.get(2).unwrap().count, 1);
    }

    #[test]
    fn an_empty_grid_rejects_the_click() {
        let mut grid = CraftingGrid::player();
        assert_rejected(&craft_once(&mut grid, &registry(), None, 64));
    }

    #[test]
    fn the_result_merges_onto_a_matching_cursor() {
        let mut grid = sticks_grid();
        let holding = stack(STICK, 10);

        assert_crafted(
            &craft_once(&mut grid, &registry(), Some(&holding), 64),
            STICK,
            14,
        );
    }

    #[test]
    fn a_nearly_full_cursor_refuses_the_result() {
        let mut grid = sticks_grid();
        let holding = stack(STICK, 62);

        assert_rejected(&craft_once(&mut grid, &registry(), Some(&holding), 64));
        assert!(!grid.is_empty(), "and the grid is untouched");
    }

    #[test]
    fn an_unstackable_result_refuses_a_loaded_cursor() {
        let mut grid = CraftingGrid::bench();
        for slot in 0..3 {
            grid.set(slot, Some(stack(PLANKS, 1)));
        }
        grid.set(4, Some(stack(STICK, 1)));
        grid.set(7, Some(stack(STICK, 1)));

        let registry = registry();
        let pickaxe = registry.result_for(&grid).expect("a pickaxe");
        assert_eq!(pickaxe.count, 1);

        assert_rejected(&craft_once(&mut grid, &registry, Some(&pickaxe), 1));
        assert_crafted(
            &craft_once(&mut grid, &registry, None, 1),
            pickaxe.item_id,
            1,
        );
    }

    #[test]
    fn a_deep_grid_crafts_as_many_times_as_its_smallest_stack() {
        let mut grid = CraftingGrid::player();
        grid.set(0, Some(stack(PLANKS, 5)));
        grid.set(2, Some(stack(PLANKS, 3)));
        let registry = registry();

        assert_eq!(grid.max_crafts(), 3);

        let mut crafted = 0;
        while let OutputClick::Crafted(_) = craft_once(&mut grid, &registry, None, 64) {
            crafted += 1;
            assert!(crafted <= 4, "it should have stopped by now");
        }

        assert_eq!(crafted, 3, "the three-deep stack is the limit");
        assert_eq!(grid.get(0).unwrap().count, 2, "two planks left over");
        assert!(grid.get(2).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coral_types::ids::items::{COBBLESTONE, PLANKS, STICK, WOOL};

    fn stack(item_id: i16, count: u8, metadata: i16) -> Option<ItemStack> {
        Some(ItemStack {
            item_id,
            count,
            metadata,
            durability: 0,
            nbt: None,
        })
    }

    fn one(item_id: i16) -> Option<ItemStack> {
        stack(item_id, 1, 0)
    }

    fn sticks() -> Recipe {
        Recipe::Shaped {
            width: 1,
            height: 2,
            pattern: vec![Some(Ingredient::any(PLANKS)), Some(Ingredient::any(PLANKS))],
            result: ItemStack {
                item_id: STICK,
                count: 4,
                metadata: 0,
                durability: 0,
                nbt: None,
            },
        }
    }

    fn asymmetric() -> Recipe {
        Recipe::Shaped {
            width: 2,
            height: 2,
            pattern: vec![
                Some(Ingredient::any(COBBLESTONE)),
                None,
                Some(Ingredient::any(COBBLESTONE)),
                Some(Ingredient::any(COBBLESTONE)),
            ],
            result: ItemStack {
                item_id: 1,
                count: 1,
                metadata: 0,
                durability: 0,
                nbt: None,
            },
        }
    }

    fn grid_from(width: usize, height: usize, cells: Vec<Option<ItemStack>>) -> CraftingGrid {
        let mut grid = CraftingGrid::new(width, height);
        for (i, cell) in cells.into_iter().enumerate() {
            grid.set(i, cell);
        }
        grid
    }

    #[test]
    fn an_empty_grid_makes_nothing() {
        let registry = RecipeRegistry::vanilla();
        assert!(registry.result_for(&CraftingGrid::player()).is_none());
        assert!(registry.result_for(&CraftingGrid::bench()).is_none());
    }

    #[test]
    fn a_shaped_recipe_matches_at_every_offset() {
        let recipe = sticks();

        for (width, height) in [(2usize, 2usize), (3, 3)] {
            let mut found = 0;
            for offset_x in 0..width {
                for offset_y in 0..(height - 1) {
                    let mut grid = CraftingGrid::new(width, height);
                    grid.set(offset_y * width + offset_x, one(PLANKS));
                    grid.set((offset_y + 1) * width + offset_x, one(PLANKS));

                    assert!(
                        recipe.matches(&grid),
                        "{width}x{height} grid, planks at column {offset_x} row {offset_y}"
                    );
                    found += 1;
                }
            }
            assert_eq!(found, width * (height - 1), "every offset was tried");
        }
    }

    #[test]
    fn an_asymmetric_recipe_matches_mirrored() {
        let recipe = asymmetric();

        let as_written = grid_from(
            2,
            2,
            vec![one(COBBLESTONE), None, one(COBBLESTONE), one(COBBLESTONE)],
        );
        assert!(recipe.matches(&as_written));

        let mirrored = grid_from(
            2,
            2,
            vec![None, one(COBBLESTONE), one(COBBLESTONE), one(COBBLESTONE)],
        );
        assert!(
            recipe.matches(&mirrored),
            "1.8 matches shaped recipes mirrored as well as as-written"
        );
    }

    #[test]
    fn a_stray_item_outside_the_pattern_blocks_the_match() {
        let recipe = sticks();

        let mut grid = CraftingGrid::bench();
        grid.set(0, one(PLANKS));
        grid.set(3, one(PLANKS));
        assert!(recipe.matches(&grid), "the two planks alone match");

        grid.set(8, one(COBBLESTONE));
        assert!(
            !recipe.matches(&grid),
            "a cobblestone in the far corner must block it"
        );
    }

    #[test]
    fn a_recipe_too_big_for_the_grid_does_not_match() {
        let three_by_three = Recipe::Shaped {
            width: 3,
            height: 3,
            pattern: vec![Some(Ingredient::any(PLANKS)); 9],
            result: ItemStack {
                item_id: 58,
                count: 1,
                metadata: 0,
                durability: 0,
                nbt: None,
            },
        };

        let full_2x2 = grid_from(2, 2, vec![one(PLANKS); 4]);
        assert!(
            !three_by_three.matches(&full_2x2),
            "a bench recipe cannot be made in the player's 2x2"
        );
    }

    #[test]
    fn shapeless_ignores_arrangement() {
        let recipe = Recipe::Shapeless {
            ingredients: vec![Ingredient::any(PLANKS), Ingredient::any(STICK)],
            result: ItemStack {
                item_id: 1,
                count: 1,
                metadata: 0,
                durability: 0,
                nbt: None,
            },
        };

        for a in 0..4 {
            for b in 0..4 {
                if a == b {
                    continue;
                }
                let mut grid = CraftingGrid::player();
                grid.set(a, one(PLANKS));
                grid.set(b, one(STICK));
                assert!(recipe.matches(&grid), "planks at {a}, stick at {b}");
            }
        }
    }

    #[test]
    fn shapeless_needs_every_ingredient_and_no_extras() {
        let recipe = Recipe::Shapeless {
            ingredients: vec![Ingredient::any(PLANKS), Ingredient::any(STICK)],
            result: ItemStack {
                item_id: 1,
                count: 1,
                metadata: 0,
                durability: 0,
                nbt: None,
            },
        };

        let missing = grid_from(2, 2, vec![one(PLANKS), None, None, None]);
        assert!(!recipe.matches(&missing), "one of the two is missing");

        let extra = grid_from(2, 2, vec![one(PLANKS), one(STICK), one(COBBLESTONE), None]);
        assert!(!recipe.matches(&extra), "a third item is present");

        let wrong = grid_from(2, 2, vec![one(PLANKS), one(PLANKS), None, None]);
        assert!(
            !recipe.matches(&wrong),
            "two planks is not planks and a stick"
        );
    }

    #[test]
    fn an_exact_metadata_ingredient_rejects_other_values() {
        let white_only = Ingredient::exact(WOOL, 0);
        let any_wool = Ingredient::any(WOOL);

        let white = ItemStack {
            item_id: WOOL,
            count: 1,
            metadata: 0,
            durability: 0,
            nbt: None,
        };
        let red = ItemStack {
            item_id: WOOL,
            count: 1,
            metadata: 14,
            durability: 0,
            nbt: None,
        };

        assert!(white_only.accepts(&white));
        assert!(
            !white_only.accepts(&red),
            "exact metadata must not match red"
        );
        assert!(any_wool.accepts(&white));
        assert!(any_wool.accepts(&red), "any metadata takes either");
    }

    #[test]
    fn crafting_takes_one_from_each_slot_however_many_it_held() {
        let mut grid = grid_from(
            2,
            2,
            vec![
                stack(PLANKS, 1, 0),
                stack(PLANKS, 64, 0),
                None,
                stack(STICK, 2, 0),
            ],
        );

        grid.consume_one_of_each();

        assert!(grid.get(0).is_none(), "a stack of one becomes empty");
        assert_eq!(grid.get(1).unwrap().count, 63);
        assert!(grid.get(2).is_none(), "an empty slot stays empty");
        assert_eq!(grid.get(3).unwrap().count, 1);
    }

    #[test]
    fn how_many_crafts_a_grid_allows_is_its_smallest_stack() {
        let grid = grid_from(
            2,
            2,
            vec![stack(PLANKS, 7, 0), stack(PLANKS, 3, 0), None, None],
        );
        assert_eq!(grid.max_crafts(), 3);

        assert_eq!(CraftingGrid::player().max_crafts(), 0, "nothing to craft");
    }

    #[test]
    fn no_two_recipes_match_the_same_grid() {
        let registry = RecipeRegistry::vanilla();

        for (i, recipe) in registry.recipes().iter().enumerate() {
            let (width, height) = recipe.min_grid();
            let mut grid = CraftingGrid::new(width.max(3), height.max(3));

            match recipe {
                Recipe::Shaped {
                    width: rw,
                    height: rh,
                    pattern,
                    ..
                } => {
                    for y in 0..*rh {
                        for x in 0..*rw {
                            if let Some(ingredient) = pattern[y * rw + x] {
                                grid.set(
                                    y * grid.width() + x,
                                    stack(ingredient.item_id, 1, ingredient.metadata.unwrap_or(0)),
                                );
                            }
                        }
                    }
                }
                Recipe::Shapeless { ingredients, .. } => {
                    for (slot, ingredient) in ingredients.iter().enumerate() {
                        grid.set(
                            slot,
                            stack(ingredient.item_id, 1, ingredient.metadata.unwrap_or(0)),
                        );
                    }
                }
            }

            let matching: Vec<usize> = registry
                .recipes()
                .iter()
                .enumerate()
                .filter(|(_, other)| other.matches(&grid))
                .map(|(j, _)| j)
                .collect();

            assert_eq!(
                matching,
                vec![i],
                "recipe {i} making {:?} is matched by {matching:?}; the later \
                 one is unreachable",
                recipe.result().item_id
            );
        }
    }

    #[test]
    fn the_registry_is_not_empty_and_every_recipe_makes_something() {
        let registry = RecipeRegistry::vanilla();
        assert!(registry.len() > 20, "only {} recipes", registry.len());

        for recipe in registry.recipes() {
            let result = recipe.result();
            assert!(result.item_id > 0, "a recipe makes item {}", result.item_id);
            assert!(result.count > 0, "a recipe makes {} items", result.count);
        }
    }
}
