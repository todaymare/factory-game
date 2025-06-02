use crate::{items::{Item, ItemKind}, structures::strct::StructureKind, voxel_world::voxel::VoxelKind, TICKS_PER_SECOND};


pub const RECIPES : &'static [Recipe<'static>] = &[
    Recipe { result: Item { amount: 1, kind: ItemKind::Structure(StructureKind::Quarry) }, requirements: &[Item { amount: 8, kind: ItemKind::IronOre }, Item { amount: 8, kind: ItemKind::CopperOre }, Item { amount: 8, kind: ItemKind::Voxel(VoxelKind::Dirt) }, Item { amount: 8, kind: ItemKind::Voxel(VoxelKind::Stone) }], time: TICKS_PER_SECOND * 3 },
    Recipe { result: Item { amount: 1, kind: ItemKind::Voxel(VoxelKind::Dirt)}, requirements: &[Item { amount: 8, kind: ItemKind::IronOre }], time: TICKS_PER_SECOND * 3 },
];


#[derive(Debug, Clone, Copy)]
pub struct Recipe<'a> {
    pub requirements: &'a [Item],
    pub result: Item,
    pub time: u32,
}


