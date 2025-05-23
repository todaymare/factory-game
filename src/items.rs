use std::collections::HashMap;

use glam::{IVec3, Vec3, Vec4};

use crate::{chunk::{draw_quad, VoxelKind}, mesh::Mesh, quad::{Direction, Quad}, structure::{Structure, StructureKind}, Game, PhysicsBody, World};


#[derive(Clone)]
pub struct DroppedItem {
    pub item: Item,
    pub body: PhysicsBody,
    pub creation_tick: u32,
}


#[derive(Clone, Copy)]
pub struct Item {
    pub amount: u32,
    pub kind  : ItemKind,
}


#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum ItemKind {
    Coal,
    CopperOre,
    IronOre,

    Structure(StructureKind),
    Voxel(VoxelKind),

    IronPlate,
}


pub struct ItemMeshes {
    meshes: HashMap<ItemKind, Mesh>,
}


impl ItemKind {
    const ALL : &[ItemKind] = &[
        ItemKind::Voxel(VoxelKind::Dirt),
        ItemKind::Voxel(VoxelKind::Stone),
        ItemKind::CopperOre,
        ItemKind::IronOre,
        ItemKind::Coal,
        ItemKind::IronPlate,
        ItemKind::Structure(StructureKind::Quarry),
        ItemKind::Structure(StructureKind::Inserter),
    ];


    pub fn max_stack_size(self) -> u32 {
        u32::MAX
    }


    pub fn as_voxel(self) -> Option<VoxelKind> {
        match self {
            ItemKind::Voxel(vk) => Some(vk),
            _ => None,
        }
    }


    pub fn as_structure(self) -> Option<StructureKind> {
        match self {
            ItemKind::Structure(structure) => Some(structure),
            _ => None,
        }
    }


    ///
    /// This function returns a Mesh of the item centred at (0, 0, 0) with
    /// a unit size
    ///
    pub fn create_mesh(self) -> Mesh {
        match self {
            _ => {
                let colour = match self {
                    ItemKind::Voxel(vk) => vk.colour(),
                    ItemKind::Structure(structure) => return structure.mesh(),
                    ItemKind::CopperOre => VoxelKind::Copper.colour(),
                    ItemKind::IronOre => VoxelKind::Iron.colour(),
                    ItemKind::Coal => VoxelKind::Coal.colour(),
                    _ => Vec4::ONE,
                };

                let mut verticies = vec![];
                let mut indicies = vec![];

                let pos = Vec3::new(-0.5, -0.5, -0.5);
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Up, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Down, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Left, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Right, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Forward, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Back, pos, colour));

                Mesh::new(verticies, indicies)
            }
        }
    }
}


impl ItemMeshes {
    pub fn new() -> Self {
        let vec = ItemKind::ALL.iter().map(|x| (*x, x.create_mesh())).collect();

        Self {
            meshes: vec,
        }
    }


    pub fn get(&self, kind: ItemKind) -> &Mesh {
        self.meshes.get(&kind).unwrap()
    }
}


impl core::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} x{}", self.kind, self.amount)
    }
}
