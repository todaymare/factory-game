pub mod data;

use crate::items::Item;

pub use data::RECIPES;
pub use data::FURNACE_RECIPES;
pub use data::crafting_recipe_inventory;


#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct Recipe {
    pub requirements: &'static [Item],
    pub result: Item,
    pub time: u32,
}



pub fn crafting_recipe_index(recipe: Recipe) -> usize {
    RECIPES.iter().enumerate().find(|x| x.1 == &recipe).unwrap().0
}


