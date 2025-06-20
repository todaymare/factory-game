use glam::Vec4;
use tracing::warn;

use crate::{items::ItemKind, TICKS_PER_SECOND};

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[repr(u8)]
pub enum Voxel {
    Air = 0,
    Dirt = 1,
    Stone = 2,

    Copper = 3,
    Iron = 4,
    Coal = 5,

    StructureBlock = 255,
}


impl Voxel {
    pub fn is_air(self) -> bool {
        matches!(self, Voxel::Air)
    }


    pub fn is_structure(self) -> bool {
        matches!(self, Voxel::StructureBlock)
    }


    pub fn is_transparent(self) -> bool { 
        matches!(self, Voxel::Air | Voxel::StructureBlock)
    }


    pub fn colour(self) -> Vec4 { 
        match self {
            Voxel::Stone => Vec4::new(0.4, 0.4, 0.4, 1.0),
            Voxel::Dirt => Vec4::new(0.30, 0.6, 0.10, 1.0),

            Voxel::Copper => Vec4::new(0.8, 0.6, 0.6, 1.0),
            Voxel::Iron => Vec4::new(0.8, 0.8, 0.8, 1.0),
            Voxel::Coal => Vec4::new(0.2, 0.2, 0.2, 1.0),

            Voxel::StructureBlock => Vec4::ZERO.with_w(1.0),
            Voxel::Air => unreachable!(),
        }
    }


    pub fn base_hardness(self) -> u32 {
        match self {
            Voxel::Dirt => TICKS_PER_SECOND / 3,
            Voxel::Stone => TICKS_PER_SECOND / 3,

            Voxel::Copper => TICKS_PER_SECOND * 2 / 3,
            Voxel::Iron => TICKS_PER_SECOND * 2 / 3,
            Voxel::Coal => TICKS_PER_SECOND * 2 / 3,
            Voxel::StructureBlock => TICKS_PER_SECOND * 1 / 3,

            Voxel::Air => unreachable!(),
        }
    }


    pub fn as_item_kind(self) -> ItemKind {
        match self {
            Voxel::Dirt => ItemKind::Voxel(self),
            Voxel::Stone => ItemKind::Voxel(self),

            Voxel::Copper => ItemKind::CopperOre,
            Voxel::Iron => ItemKind::IronOre,
            Voxel::Coal => ItemKind::Coal,

            Voxel::StructureBlock => unreachable!(),
            Voxel::Air => unreachable!(),
        }
    }

}


