use glam::Vec4;

use crate::{items::ItemKind, TICKS_PER_SECOND};

#[derive(Debug, Clone, Copy)]
pub struct Voxel {
    pub kind: VoxelKind,
}


#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[repr(u8)]
pub enum VoxelKind {
    Dirt,
    Stone,

    Copper,
    Iron,
    Coal,

    StructureBlock,
    Air,
}


impl VoxelKind {
    pub fn is_air(self) -> bool {
        matches!(self, VoxelKind::Air)
    }


    pub fn is_structure(self) -> bool {
        matches!(self, VoxelKind::StructureBlock)
    }


    pub fn is_transparent(self) -> bool { 
        matches!(self, VoxelKind::Air | VoxelKind::StructureBlock)
    }


    pub fn colour(self) -> Vec4 { 
        match self {
            VoxelKind::Stone => Vec4::new(0.4, 0.4, 0.4, 1.0),
            VoxelKind::Dirt => Vec4::new(0.30, 0.6, 0.10, 1.0),

            VoxelKind::Copper => Vec4::new(0.8, 0.6, 0.6, 1.0),
            VoxelKind::Iron => Vec4::new(0.8, 0.8, 0.8, 1.0),
            VoxelKind::Coal => Vec4::new(0.2, 0.2, 0.2, 1.0),

            VoxelKind::StructureBlock => Vec4::ZERO.with_w(1.0),
            VoxelKind::Air => unreachable!(),
        }
    }


    pub fn to_u8(self) -> u8 {
        match self {
            VoxelKind::Dirt => 1,
            VoxelKind::Stone => 2,
            VoxelKind::Copper => 3,
            VoxelKind::Iron => 4,
            VoxelKind::Coal => 5,
            VoxelKind::StructureBlock => 0,
            VoxelKind::Air => 0,
        }
    }



    pub fn from_u8(u8: u8) -> VoxelKind {
        match u8 {
            0 => Self::Air,
            1 => Self::Dirt,
            2 => Self::Stone,
            3 => Self::Copper,
            4 => Self::Iron,
            5 => Self::Coal,

            _ => {
                println!("invalid block id '{u8}'");
                Self::Air
            }
        }
    }


    pub fn base_hardness(self) -> u32 {
        match self {
            VoxelKind::Dirt => TICKS_PER_SECOND / 3,
            VoxelKind::Stone => TICKS_PER_SECOND / 3,
            VoxelKind::Copper => TICKS_PER_SECOND * 2 / 3,
            VoxelKind::Iron => TICKS_PER_SECOND * 2 / 3,
            VoxelKind::Coal => TICKS_PER_SECOND * 2 / 3,
            VoxelKind::StructureBlock => TICKS_PER_SECOND * 2 / 3,

            VoxelKind::Air => unreachable!(),
        }
    }


    pub fn as_item_kind(self) -> ItemKind {
        match self {
            VoxelKind::Dirt => ItemKind::Voxel(self),
            VoxelKind::Stone => ItemKind::Voxel(self),

            VoxelKind::Copper => ItemKind::CopperOre,
            VoxelKind::Iron => ItemKind::IronOre,
            VoxelKind::Coal => ItemKind::Coal,

            VoxelKind::StructureBlock => unreachable!(),
            VoxelKind::Air => unreachable!(),
        }
    }

}


