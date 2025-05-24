use std::{hash::{DefaultHasher, Hash, Hasher}, time::Instant};

use glam::{IVec3, Vec3, Vec4};
use rand::{Rng, SeedableRng};

use crate::{items::ItemKind, mesh::{Mesh, Vertex}, quad::{Direction, Quad}, TICKS_PER_SECOND};

pub const CHUNK_SIZE : usize = 32;


#[derive(Debug)]
pub struct Chunk {
    data: [Voxel; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
    pub mesh: Option<Mesh>,
}


#[derive(Debug, Clone, Copy)]
pub struct Voxel {
    pub kind: VoxelKind,
}


#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
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
            VoxelKind::Dirt => Vec4::new(0.54, 0.27, 0.07, 1.0),

            VoxelKind::Copper => Vec4::new(0.8, 0.6, 0.6, 1.0),
            VoxelKind::Iron => Vec4::new(0.8, 0.8, 0.8, 1.0),
            VoxelKind::Coal => Vec4::new(0.2, 0.2, 0.2, 1.0),

            VoxelKind::StructureBlock => Vec4::ZERO.with_w(1.0),
            VoxelKind::Air => unreachable!(),
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


impl Chunk {
    pub fn empty_chunk() -> Chunk {
        Chunk {
            data: [Voxel { kind: VoxelKind::Air }; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
            mesh: None,
        }
    }


    pub fn generate(position: IVec3) -> Chunk {
        let mut hasher = DefaultHasher::new();
        position.hash(&mut hasher);
        let hash = hasher.finish();
        let mut rng = rand::rngs::SmallRng::seed_from_u64(hash);

        let mut chunk = Chunk::empty_chunk();
    
        for z in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let chunk_local_position = IVec3::new(x as i32, y as i32, z as i32);
                    let global_position = position * CHUNK_SIZE as i32 + chunk_local_position;

                    let mut kind = VoxelKind::Air;
                    if global_position.y == 0 {
                        kind = VoxelKind::Dirt;
                    } else if global_position.y > 0 {
                        kind = VoxelKind::Air;
                    } else {
                        let rand : f32 = rng.random_range(0.0..1.0);
                        if rand < 0.5 {
                            kind = VoxelKind::Stone;
                        } else if rand < 0.70 {
                            kind = VoxelKind::Coal;
                        } else if rand < 0.85 {
                            kind = VoxelKind::Iron;
                        } else if rand <= 1.0 {
                            kind = VoxelKind::Copper;
                        }
                    }


                    let voxel = Voxel { kind };
                    *chunk.get_mut(chunk_local_position) = voxel;

                }
            }
        }
        chunk
    }


    pub fn mesh(&mut self) -> &Mesh {
        const FACE_DIRECTIONS: [(Direction, (i32, i32, i32)); 6] = [
            (Direction::Up,      ( 0,  1,  0)),
            (Direction::Down,    ( 0, -1,  0)),
            (Direction::Right,    (-1,  0,  0)),
            (Direction::Left,   ( 1,  0,  0)),
            (Direction::Forward, ( 0,  0,  1)),
            (Direction::Back,    ( 0,  0, -1)),
        ];

        if self.mesh.is_none() {
            let time = Instant::now();
            let mut verticies = vec![];
            let mut indicies = vec![];

            for z in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        let voxel = *self.get_usize(x, y, z);

                        if voxel.kind.is_transparent() { continue }

                        let pos = Vec3::new(x as f32, y as f32, z as f32);

                        for (dir, (dx, dy, dz)) in FACE_DIRECTIONS.iter() {
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            let nz = z as i32 + dz;

                            let is_out_of_bounds = nx < 0 || nx >= CHUNK_SIZE as i32
                                                || ny < 0 || ny >= CHUNK_SIZE as i32
                                                || nz < 0 || nz >= CHUNK_SIZE as i32;

                            let should_draw = if is_out_of_bounds {
                                true
                            } else {
                                self.get_usize(nx as usize, ny as usize, nz as usize).kind.is_transparent()
                            };

                            if should_draw {
                                draw_quad(&mut verticies, &mut indicies,
                                          Quad::from_direction(*dir, pos, voxel.kind.colour()));
                            }
                        }
                    }
                }
            }

            let mesh = Mesh::new(verticies, indicies);
            self.mesh = Some(mesh);
            println!("remeshed chunk in {}ms", time.elapsed().as_millis_f64());
        }

        self.mesh.as_ref().unwrap()

    }


    pub fn get_mut(&mut self, pos: IVec3) -> &mut Voxel {
        self.get_mut_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    pub fn get_mut_usize(&mut self, x: usize, y: usize, z: usize) -> &mut Voxel {
        self.mesh = None;
        &mut self.data[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x]
    }


    pub fn get(&self, pos: IVec3) -> &Voxel {
        self.get_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    pub fn get_usize(&self, x: usize, y: usize, z: usize) -> &Voxel {
        &self.data[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x]
    }
}


pub fn draw_quad(verticies: &mut Vec<Vertex>, indicies: &mut Vec<u32>, quad: Quad) {
    let normal = match quad.direction {
        Direction::Left => Vec3::new(1.0, 0.0, 0.0),
        Direction::Right => Vec3::new(-1.0, 0.0, 0.0),
        Direction::Down => Vec3::new(0.0, -1.0, 0.0),
        Direction::Up => Vec3::new(0.0, 1.0, 0.0),
        Direction::Back => Vec3::new(0.0, 0.0, -1.0),
        Direction::Forward => Vec3::new(0.0, 0.0, 1.0),
    };

    let k = verticies.len() as u32;
    let mut i = 0;
    for corner in quad.corners {
        let mut colour = quad.color;
        colour = colour * 0.9 + colour * (i as f32 * 0.1);
        colour.w = quad.color.w;
        verticies.push(Vertex::new(Vec3::new(corner[0] as f32, corner[1] as f32, corner[2] as f32), normal, colour));
        i += 1;
    }


    indicies.extend_from_slice(&[k, k+1, k+2, k+2, k+3, k]);
}
