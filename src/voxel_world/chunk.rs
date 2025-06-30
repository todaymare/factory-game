use std::{hash::Hash, i32, num::{NonZero, NonZeroI16, NonZeroU32}, simd::{cmp::SimdPartialEq, u8x64}, sync::Arc};

use glam::{DVec2, IVec2, IVec3, Vec3Swizzles};
use libnoise::{Generator, ImprovedPerlin, Simplex, Source};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use sti::{hash::fxhash::FxHasher64, key::Key};

use crate::{constants::{CHUNK_SIZE, CHUNK_SIZE_P3}, octree::NodeId, renderer::ChunkIndex, voxel_world::voxel::Voxel};

use super::mesh::ChunkMesh;

#[derive(Debug)]
pub struct Chunk {
    pub data: Arc<ChunkData>,
    pub is_dirty: bool,
    pub meshes: Option<NodeId>,
    pub current_mesh: u32,
    pub version: NonZeroU32,
}


#[derive(Debug, Clone)]
pub struct ChunkData {
    pub data: [Voxel; CHUNK_SIZE_P3],
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
            biomes: Source::improved_perlin(seed),
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

            let base = self.perlin.sample([x * base_scale, z * base_scale]) * 40.0 + 40.0;

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
            data: Arc::new(ChunkData::empty()),
            is_dirty: false,
            meshes: None,
            // we still want to remesh initially
            version: NonZero::new(1).unwrap(),
            current_mesh: 0,
        }
    }


    pub fn generate(pos: IVec3, noise: &Noise) -> Chunk {
        let mut data = ChunkData::empty();

        let mut height_map = [[0; CHUNK_SIZE]; CHUNK_SIZE];
        let mut max_height = i32::MIN;
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let global_pos = (pos * CHUNK_SIZE as i32).xz() + IVec2::new(x as i32, z as i32);

                let height = noise.sample(global_pos.as_dvec2());
                let height = height as i32;

                max_height = max_height.max(height);


                height_map[x][z] = height;
            }
        }

        let skip = (pos.y * CHUNK_SIZE as i32) > max_height;
    
        if !skip {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    for y in 0..CHUNK_SIZE {
                        let chunk_local_position = IVec3::new(x as i32, y as i32, z as i32);
                        let global_position = pos * CHUNK_SIZE as i32 + chunk_local_position;

                        let height = height_map[x][z];

                        let kind;
                        if global_position.y == height {
                            kind = Voxel::Dirt;
                        } else if global_position.y > height {
                            continue
                        } else {
                            kind = Voxel::Stone;
                        }

                        *data.get_mut_usize(x, y, z) = kind;
                    }
                }
            }

            let mut hasher = FxHasher64::new();
            pos.hash(&mut hasher);
            let mut rng = rand::rngs::SmallRng::seed_from_u64(hasher.hash);


            let vein_count = rng.random_range(64..128);
            let mut buff : Vec<IVec3> = vec![];
            let mut i = 0;
            while i < vein_count {
                i += 1;
                buff.clear();
                buff.push(random_pos(&mut rng));

                let vein_block = match rng.random_range(0..100) {
                    0..20 => Voxel::Coal,
                    20..60 => Voxel::Copper,
                    _ => Voxel::Iron,
                };

                let vein_size = rng.random_range(64..168);
                let mut j = 0;
                while buff.len() < vein_size && j < vein_size * 2 {
                    j += 1;

                    let base = buff[rng.random_range(0..buff.len())];
                    let x = rng.random_range(-1..=1);
                    let y = rng.random_range(-1..=1);
                    let z = rng.random_range(-1..=1);
                    let pos = base + IVec3::new(x, y, z);

                    if pos.x < 0 || pos.y < 0 || pos.z < 0
                        || pos.x >= CHUNK_SIZE as i32
                        || pos.y >= CHUNK_SIZE as i32
                        || pos.z >= CHUNK_SIZE as i32 {
                        j -= 1;
                        continue;
                    }
                        

                    let voxel = data.get_mut(pos);
                    if *voxel == Voxel::Stone {
                        *voxel = vein_block;
                        buff.push(pos);
                    }
                }

                if buff.is_empty() {
                    i -= 1;
                }
            }

        }

        let chunk = Chunk {
            data: data.into(),
            is_dirty: true,
            meshes: None,
            current_mesh: 0,
            version: NonZero::new(1).unwrap(),
        };
        chunk
    }


    pub fn get_mut(&mut self, pos: IVec3) -> &mut Voxel {
        self.get_mut_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    pub fn get_mut_usize(&mut self, x: usize, y: usize, z: usize) -> &mut Voxel {
        Arc::make_mut(&mut self.data).get_mut_usize(x, y, z)
    }


    pub fn get(&self, pos: IVec3) -> Voxel {
        self.get_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    pub fn get_usize(&self, x: usize, y: usize, z: usize) -> Voxel {
        self.data.get_usize(x, y, z)
    }

}


fn is_all_zero_simd(data: &[u8]) -> bool {
    data.chunks_exact(64).all(|chunk| {
        let simd = u8x64::from_slice(chunk);
        simd.simd_eq(u8x64::splat(0)).all()
    })
}


fn random_pos(rng: &mut SmallRng) -> IVec3 {
    IVec3::new(rng.random_range(0..CHUNK_SIZE) as _, rng.random_range(0..CHUNK_SIZE) as _, rng.random_range(0..CHUNK_SIZE) as _)
}


impl ChunkData {
    pub fn from_bytes(bytes: [u8; CHUNK_SIZE_P3]) -> Self {
        let voxels = unsafe { core::mem::transmute(bytes) };
        Self::from_voxels(voxels)
    }


    pub fn from_voxels(voxels: [Voxel; CHUNK_SIZE_P3]) -> Self {
        Self {
            data: voxels,
        }
    }


    pub fn empty() -> Self {
        Self::from_voxels([Voxel::Air; CHUNK_SIZE_P3])
    }


    pub fn is_empty(&self) -> bool {
        is_all_zero_simd(self.as_bytes())
    }


    pub fn as_bytes(&self) -> &[u8; CHUNK_SIZE_P3] {
        unsafe {
            core::mem::transmute::<_, &[u8; CHUNK_SIZE_P3]>
                (&self.data)
        }
    }


    #[inline(always)]
    pub fn get_mut(&mut self, pos: IVec3) -> &mut Voxel {
        self.get_mut_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    #[inline(always)]
    pub fn get_mut_usize(&mut self, x: usize, y: usize, z: usize) -> &mut Voxel {
        &mut self.data[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x]
    }


    #[inline(always)]
    pub fn get(&self, pos: IVec3) -> Voxel {
        self.get_usize(pos.x as usize, pos.y as usize, pos.z as usize)
    }


    #[inline(always)]
    pub fn get_usize(&self, x: usize, y: usize, z: usize) -> Voxel {
        self.data[z * CHUNK_SIZE * CHUNK_SIZE + y * CHUNK_SIZE + x]
    }
}


