use std::{hash::{DefaultHasher, Hash, Hasher}, sync::Arc, time::Instant};

use glam::{DVec2, IVec3, Vec2, Vec3Swizzles};
use libnoise::{Generator, ImprovedPerlin, Perlin, RidgedMulti, Simplex, Source};
use rand::{Rng, SeedableRng};

use crate::{perlin::PerlinNoise, voxel_world::voxel::VoxelKind};

use super::{mesh::VoxelMesh, voxel::Voxel};


pub const CHUNK_SIZE : usize = 32;

#[derive(Debug)]
pub struct Chunk {
    pub data: Arc<ChunkData>,
    pub is_dirty: bool,
    pub mesh: Option<VoxelMesh>,
    pub mesh_state: MeshState,
    pub persistent: bool,
}


#[derive(Debug, Clone)]
pub struct ChunkData {
    pub data: [Voxel; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
}


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MeshState {
    ShouldUpdate,
    Updating,
    Okay,
}


pub struct Noise {
    perlin: ImprovedPerlin<2>,
    simplex: Simplex<2>,
    biomes: ImprovedPerlin<2>,
}

impl Noise {
    pub fn new(seed: u64) -> Self {
        Self {
            perlin: Source::improved_perlin(seed),
            simplex: Source::simplex(seed),
            biomes: Source::improved_perlin(seed+69),
        }
    }


    pub fn sample(&self, pos: DVec2) -> f64 {
        let x = pos.x + 10_000.0;
        let z = pos.y + 10_000.0;
        let biome = self.biomes.sample([x * 0.0055, z * 0.0055]);
        let biome = (biome + 1.0) * 0.5;

        let giant_mountain_height = {
            let base_scale = 0.0003;
            let detail_scale = 0.02;

            let base = self.perlin.sample([x * base_scale, z * base_scale]) * 360.0 + 160.0;

            let detail = self.simplex.sample([x * detail_scale + 1337.0, z * detail_scale + 420.0]) * 16.0;
            base + detail
        };


        let mountain_height = {
            let base_scale = 0.003;
            let detail_scale = 0.02;

            let base = self.perlin.sample([x * base_scale, z * base_scale]) * 80.0 + 40.0;

            let detail = self.simplex.sample([x * detail_scale + 1337.0, z * detail_scale + 420.0]) * 8.0;
            base + detail
        };


        let plateau_height = {
            let base_scale = 0.005;
            let detail_scale = 0.02;

            let base = self.perlin.sample([x * base_scale, z * base_scale]) * 3.0;

            let detail = self.simplex.sample([x * detail_scale + 1337.0, z * detail_scale + 420.0]) * 1.0;
            base + detail
        };

        let height = if biome < 0.5 {
            let t = (biome / 0.5).clamp(0.0, 1.0);
            lerp(plateau_height, mountain_height, smoothstep(t))
        } else {
            let t = ((biome - 0.5) / 0.5).clamp(0.0, 1.0);
            lerp(mountain_height, giant_mountain_height, smoothstep(t))
        };

        height
    }

}


fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a * (1.0 - t) + b * t
}


fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}


impl Chunk {
    pub fn empty_chunk() -> Chunk {
        Chunk {
            data: Arc::new(ChunkData { data: [Voxel { kind: VoxelKind::Air }; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE] }),
            is_dirty: false,
            mesh: None,
            mesh_state: MeshState::ShouldUpdate,
            persistent: false,
        }
    }


    pub fn generate(pos: IVec3, noise: &Noise) -> Chunk {
        let mut hasher = DefaultHasher::new();
        pos.hash(&mut hasher);
        let hash = hasher.finish();
        let mut rng = rand::rngs::SmallRng::seed_from_u64(hash);
        let mut chunk = Chunk::empty_chunk();
        let data = Arc::make_mut(&mut chunk.data);
    
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    let chunk_local_position = IVec3::new(x as i32, y as i32, z as i32);
                    let global_position = pos * CHUNK_SIZE as i32 + chunk_local_position;


                    let height = noise.sample(global_position.xz().as_dvec2());
                    let height = height as i32;

                    let mut kind = VoxelKind::Air;
                    if global_position.y == height {
                        kind = VoxelKind::Dirt;
                    } else if global_position.y > height {
                        continue
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
                    *data.get_mut(chunk_local_position) = voxel;
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
        Arc::make_mut(&mut self.data).get_mut_usize(x, y, z)
    }


    pub fn get(&self, pos: IVec3) -> &Voxel {
        self.get_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    pub fn get_usize(&self, x: usize, y: usize, z: usize) -> &Voxel {
        self.data.get_usize(x, y, z)
    }
}


impl ChunkData {
    pub fn empty() -> Self {
        Self {
            data: [Voxel { kind: VoxelKind::Air }; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
        }
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



