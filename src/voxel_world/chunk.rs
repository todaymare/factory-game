use std::hash::{DefaultHasher, Hash, Hasher};

use glam::IVec3;
use libnoise::{Generator, Perlin, Simplex};
use rand::{Rng, SeedableRng};

use crate::{perlin::PerlinNoise, voxel_world::voxel::VoxelKind};

use super::{mesh::VoxelMesh, voxel::Voxel};


pub const CHUNK_SIZE : usize = 32;


#[derive(Debug)]
pub struct Chunk {
    pub data: [Voxel; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
    pub is_dirty: bool,
    pub mesh: VoxelMesh,
    pub mesh_state: MeshState,
}


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MeshState {
    ShouldUpdate,
    Updating,
    Okay,
}



impl Chunk {
    pub fn empty_chunk() -> Chunk {
        Chunk {
            data: [Voxel { kind: VoxelKind::Air }; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
            is_dirty: false,
            mesh: VoxelMesh::new(vec![], vec![]),
            mesh_state: MeshState::ShouldUpdate,
        }
    }


    pub fn generate(pos: IVec3, perlin: &Perlin<2>) -> Chunk {
        let mut hasher = DefaultHasher::new();
        pos.hash(&mut hasher);
        let hash = hasher.finish();
        let mut rng = rand::rngs::SmallRng::seed_from_u64(hash);
        let mut chunk = Chunk::empty_chunk();
    
        for z in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let chunk_local_position = IVec3::new(x as i32, y as i32, z as i32);
                    let global_position = pos * CHUNK_SIZE as i32 + chunk_local_position;

                    let sample_point = [
                        global_position.x as f64 * 0.001 + 100_000.0,
                        global_position.z as f64 * 0.001 + 100_000.0,
                    ];
                    let height = perlin.sample(sample_point);
                    let height = (height*128.0).floor() as i32;

                    let mut kind = VoxelKind::Air;
                    if global_position.y == height {
                        kind = VoxelKind::Dirt;
                    } else if global_position.y > height {
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

        chunk.is_dirty = true;
        chunk
    }


    pub fn get_mut(&mut self, pos: IVec3) -> &mut Voxel {
        self.get_mut_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    pub fn get_mut_usize(&mut self, x: usize, y: usize, z: usize) -> &mut Voxel {
        &mut self.data[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x]
    }


    pub fn get(&self, pos: IVec3) -> &Voxel {
        self.get_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    pub fn get_usize(&self, x: usize, y: usize, z: usize) -> &Voxel {
        &self.data[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x]
    }
}



