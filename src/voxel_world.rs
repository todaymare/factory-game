pub mod chunk;
pub mod voxel;
pub mod mesh;
pub mod chunker;

use std::{fs::{self}, hint::spin_loop, ops::Bound, sync::Arc, time::Instant};

use chunk::{ChunkData, Noise};
use chunker::{Chunker, WorldChunkPos};
use glam::{DVec3, IVec3, UVec3, Vec3};
use mesh::{ChunkDataRef, ChunkFaceMesh, ChunkMeshFramedata, ChunkMeshes, ChunkQuadInstance, VoxelMeshIndex};
use save_format::byte::ByteReader;
use tracing::{error, info, warn};
use voxel::Voxel;
use wgpu::util::StagingBelt;

use crate::{constants::{CHUNK_SIZE, CHUNK_SIZE_I32, REGION_SIZE}, free_list::FreeKVec, items::{DroppedItem, Item}, renderer::{gpu_allocator::GPUAllocator, ssbo::SSBO}, structures::{strct::{InserterState, StructureData}, StructureId, Structures}, voxel_world::chunk::Chunk, PhysicsBody};


pub struct VoxelWorld {
    pub structure_blocks: sti::hash::HashMap<IVec3, StructureId>,
    pub dropped_items: Vec<DroppedItem>,
    pub chunker: Chunker,
}


pub const SURROUNDING_OFFSETS : &[IVec3] = &[
    IVec3::new( 1,  0,  0),
    IVec3::new(-1,  0,  0),
    IVec3::new( 0,  1,  0),
    IVec3::new( 0, -1,  0),
    IVec3::new( 0,  0,  1),
    IVec3::new( 0,  0, -1),
];



impl VoxelWorld {
    pub fn new() -> Self {
        Self {
            chunker: Chunker::new(),
            structure_blocks: sti::hash::HashMap::new(),
            dropped_items: vec![],
        }

    }


    pub fn process(&mut self, free_list: &mut FreeKVec<VoxelMeshIndex, ChunkMeshFramedata>, instance_allocator: &mut GPUAllocator<ChunkQuadInstance>) {
        self.chunker.process_mesh_queue(3, free_list);
        self.chunker.process_chunk_queue(3);
        self.chunker.process_chunk_jobs(3);
        self.chunker.process_mesh_unload_queue(3, free_list, instance_allocator);
    }


    pub fn get_chunk(&mut self, pos: IVec3) -> &Chunk {
        &*self.ensure_chunk_exists(pos)
    }


    pub fn get_chunk_mut(&mut self, pos: IVec3) -> &mut Chunk {
        self.ensure_chunk_exists(pos);
        self.chunker.get_mut_chunk(WorldChunkPos(pos)).unwrap()
    }


    pub fn try_get_chunk(&mut self, pos: IVec3) -> Option<&Chunk> {
        self.chunker.get_chunk_or_queue(WorldChunkPos(pos)).map(|x| &*x)
    }


    pub fn ensure_chunk_exists(&mut self, pos: IVec3) -> &Chunk {
        self.chunker.get_chunk_or_generate(WorldChunkPos(pos))
    }


    pub fn chunk_creation_job(pos: IVec3, noise: &Noise) -> Chunk {
        let path = format!("saves/chunks/{pos}.chunk");
        let chunk = match fs::read(&path) {
            Ok(ref v) if let Some(mut byte_reader) = ByteReader::new(&v) => {
                let mut chunk = Chunk::empty_chunk();
                let data = ChunkData::from_bytes(byte_reader.read().unwrap());
                if !data.is_empty() {
                    chunk.set_chunk_data(data);
                }

                chunk.is_dirty = false;
                chunk
            },


            _ => {
                Chunk::generate(pos, noise)
            }
        };

        chunk

    }


    pub fn try_get_mesh(&mut self, pos: IVec3) -> Option<&ChunkMeshes> {
        self.chunker.get_mesh_or_queue(WorldChunkPos(pos))
    }


    pub fn get_voxel(&mut self, pos: IVec3) -> Voxel {
        let (chunk_pos, chunk_local_pos) = split_world_pos(pos);

        self.get_chunk(chunk_pos).get(chunk_local_pos)
    }


    pub fn get_voxel_mut(&mut self, pos: IVec3) -> &mut Voxel {
        let (chunk_pos, chunk_local_pos) = split_world_pos(pos);
        self.get_chunk_mut(chunk_pos).get_mut(chunk_local_pos)
    }


    pub fn block_item(&mut self, structures: &Structures, pos: IVec3) -> Item {
        let voxel = self.get_voxel(pos);

        if voxel.is_structure() {
            let structure_id = *self.structure_blocks.get(&pos).unwrap();
            let structure = structures.get(structure_id);
            let kind = structure.data.as_kind().item_kind();
            Item { amount: 1, kind }
        } else {
            let kind = voxel;
            let item = Item { amount: 1, kind: kind.as_item_kind() };
            item
        }

    }


    pub fn drop_item(&mut self, item: Item, pos: DVec3) {
        self.dropped_items.push(DroppedItem::new(item, pos));
    }


    pub fn break_block(&mut self, structures: &mut Structures, pos: IVec3) -> Item {
        let voxel = self.get_voxel_mut(pos);

        let item = if voxel.is_structure() {
            let structure_id = *self.structure_blocks.get(&pos).unwrap();
            let structure = structures.remove(structure_id);
            let placement_origin = structure.position - structure.data.as_kind().origin(structure.direction);
            
            let blocks = structure.data.as_kind().blocks(structure.direction);
            let kind = structure.data.as_kind().item_kind();

            for offset in blocks {
                let pos = placement_origin + offset;

                *self.get_voxel_mut(pos) = Voxel::Air;
                self.structure_blocks.remove(&pos).unwrap();
            }


            let mut cursor = structures.work_queue.entries.lower_bound_mut(Bound::Unbounded);
            while let Some(((_, id), ())) = cursor.next() {
                if *id != structure_id { continue }

                cursor.remove_prev();
            }


            if let Some(inv) = structure.inventory {
                for item in &inv.slots {
                    let Some(item) = item
                    else { continue };
                    self.dropped_items.push(DroppedItem::new(*item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)));
                }
            }


            match structure.data {
                StructureData::Inserter { state: InserterState::Placing(item), .. } => {
                    self.dropped_items.push(DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)));
                }

                StructureData::Furnace { input, output } => {
                    if let Some(item) = input {
                        self.dropped_items.push(DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)));
                    }
                    if let Some(item) = output {
                        self.dropped_items.push(DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)));
                    }
                }
                _ => (),
            }


            Item { amount: 1, kind }

        } else {
            let kind = *voxel;
            let item = Item { amount: 1, kind: kind.as_item_kind() };
            *voxel = Voxel::Air;
            item
        };

        item
    }

    pub fn raycast_voxel(&mut self, start: DVec3, dir: Vec3, max_dist: f32) -> Option<(IVec3, IVec3)> {
        let mut pos = start.floor().as_ivec3();
        let step = dir.signum();

        let delta = Vec3::new(
            (1.0 / dir.x).abs(),
            (1.0 / dir.y).abs(),
            (1.0 / dir.z).abs()
        );


        let mut t_max = {
            let fract = start - pos.as_dvec3();
            DVec3::new(
                if dir.x > 0.0 { 1.0 - fract.x } else { fract.x } * delta.x as f64,
                if dir.y > 0.0 { 1.0 - fract.y } else { fract.y } * delta.y as f64,
                if dir.z > 0.0 { 1.0 - fract.z } else { fract.z } * delta.z as f64,
            )
        };


        let mut dist = 0.0;
        let mut last_move = Vec3::ZERO;

        while dist < max_dist as _ {
            let voxel = self.get_voxel(pos);

            let is_solid = !voxel.is_air();

            if is_solid {
                return Some((pos, -last_move.normalize().as_ivec3()));
            }

            if t_max.x < t_max.y && t_max.x < t_max.z {
                pos.x += step.x as i32;
                dist = t_max.x;
                t_max.x += delta.x as f64;
                last_move = Vec3::new(step.x, 0.0, 0.0);
            } else if t_max.y < t_max.z {
                pos.y += step.y as i32;
                dist = t_max.y;
                t_max.y += delta.y as f64;
                last_move = Vec3::new(0.0, step.y, 0.0);
            } else {
                pos.z += step.z as i32;
                dist = t_max.z;
                t_max.z += delta.z as f64;
                last_move = Vec3::new(0.0, 0.0, step.z);
            }

        }
        None
    }


    pub fn move_physics_body(&mut self, delta_time: f32, physics_body: &mut PhysicsBody) {
        physics_body.velocity.y -= 9.8 * delta_time;

        let mut position = physics_body.position;

        physics_body.velocity.x *= 1.0 - 10.0 * delta_time;
        physics_body.velocity.z *= 1.0 - 10.0 * delta_time;

        for axis in 0..3 {
            let mut new_position = position;
            new_position[axis] += (physics_body.velocity[axis] * delta_time) as f64;

            let min = (new_position - (physics_body.aabb_dims * 0.5).as_dvec3()).floor().as_ivec3();
            let max = (new_position + (physics_body.aabb_dims * 0.5).as_dvec3()).ceil().as_ivec3();

            let mut collided = false;

            for x in min.x..max.x {
                for y in min.y..max.y {
                    for z in min.z..max.z {
                        let voxel_pos = IVec3::new(x, y, z);
                        if !self.get_voxel(voxel_pos).is_air() {
                            collided = true;
                            break;
                        }
                    }
                    if collided { break; }
                }
                if collided { break; }
            }

            if collided {
                physics_body.velocity[axis] = 0.0;
            } else {
                position[axis] = new_position[axis];
            }
        }


        while !self.get_voxel(position.floor().as_ivec3()).is_air() {
            position.y += 1.0;
        }

        physics_body.position = position;
    }



    pub fn save(&mut self) {
        warn!("voxel-save-system: saving the world..");
        let time = Instant::now();
        while self.chunker.chunk_load_queue_len() > 0 { self.chunker.process_chunk_queue(128); }
        while self.chunker.chunk_active_jobs_len() > 0 { self.chunker.process_chunk_jobs(512); }

        let chunks = self.chunker.iter_chunks().map(|x| x.0).collect::<Vec<_>>();
        for pos in chunks { self.chunker.save_chunk(pos); }
        while self.chunker.chunk_save_jobs.fetch_add(0, std::sync::atomic::Ordering::SeqCst) > 0 { spin_loop(); }

        info!("voxel-save-system: saved the world in {:?}", time.elapsed());
    }



    pub fn greedy_mesh(c: [VoxelMeshIndex; 6], pos: IVec3, chunks: ChunkDataRef) -> [Vec<ChunkQuadInstance>; 6]{
        let [west, east] = Self::greedy_mesh_dir(c[0], c[3], &chunks, pos, 0);
        let [up, down] = Self::greedy_mesh_dir(c[1], c[4], &chunks, pos, 1);
        let [north, south] = Self::greedy_mesh_dir(c[2], c[5], &chunks, pos, 2);
        [west, up, north, east, down, south]
    }


    pub fn greedy_mesh_dir(
        front_chunk_index: VoxelMeshIndex,
        back_chunk_index: VoxelMeshIndex,
        chunks: &ChunkDataRef,
        pos: IVec3,
        d: usize
    ) -> [Vec<ChunkQuadInstance>; 2] {
        // offsets of corners per vertex per direction
        const AO_OFFSETS: &[[[IVec3; 3]; 4]; 6] = &[
            // X
            [
                [IVec3::new(0, -1, 0), IVec3::new(0, 0, -1), IVec3::new(0, -1, -1)],
                [IVec3::new(0, -1, 0), IVec3::new(0, 0, 1),  IVec3::new(0, -1, 1)],
                [IVec3::new(0, 1, 0),  IVec3::new(0, 0, -1), IVec3::new(0, 1, -1)],
                [IVec3::new(0, 1, 0),  IVec3::new(0, 0, 1),  IVec3::new(0, 1, 1)],
            ],
            // Y
            [
                [IVec3::new(-1, 0, 0), IVec3::new(0, 0, -1), IVec3::new(-1, 0, -1)],
                [IVec3::new(1, 0, 0),  IVec3::new(0, 0, -1), IVec3::new(1, 0, -1)],
                [IVec3::new(-1, 0, 0), IVec3::new(0, 0, 1),  IVec3::new(-1, 0, 1)],
                [IVec3::new(1, 0, 0),  IVec3::new(0, 0, 1),  IVec3::new(1, 0, 1)],
            ],
            // Z
            [
                [IVec3::new(-1, 0, 0), IVec3::new(0, -1, 0), IVec3::new(-1, -1, 0)],
                [IVec3::new(-1, 0, 0), IVec3::new(0, 1, 0),  IVec3::new(-1, 1, 0)],
                [IVec3::new(1, 0, 0),  IVec3::new(0, -1, 0), IVec3::new(1, -1, 0)],
                [IVec3::new(1, 0, 0),  IVec3::new(0, 1, 0),  IVec3::new(1, 1, 0)],
            ],
            // X-
            [
                [IVec3::new(0, -1, 0), IVec3::new(0, 0, -1), IVec3::new(0, -1, -1)],
                [IVec3::new(0, 1, 0), IVec3::new(0, 0, -1),  IVec3::new(0, 1, -1)],
                [IVec3::new(0, -1, 0),  IVec3::new(0, 0, 1), IVec3::new(0, -1, 1)],
                [IVec3::new(0, 1, 0),  IVec3::new(0, 0, 1),  IVec3::new(0, 1, 1)],
            ],
            // Y-
            [
                [IVec3::new(-1, 0, 0), IVec3::new(0, 0, -1), IVec3::new(-1, 0, -1)],
                [IVec3::new(-1, 0, 0), IVec3::new(0, 0, 1),  IVec3::new(-1, 0, 1)],
                [IVec3::new(1, 0, 0),  IVec3::new(0, 0, -1), IVec3::new(1, 0, -1)],
                [IVec3::new(1, 0, 0),  IVec3::new(0, 0, 1),  IVec3::new(1, 0, 1)],
            ],
            // Z-
            [
                [IVec3::new(-1, 0, 0), IVec3::new(0, -1, 0), IVec3::new(-1, -1, 0)],
                [IVec3::new(1, 0, 0),  IVec3::new(0, -1, 0), IVec3::new(1, -1, 0)],
                [IVec3::new(-1, 0, 0), IVec3::new(0, 1, 0),  IVec3::new(-1, 1, 0)],
                [IVec3::new(1, 0, 0),  IVec3::new(0, 1, 0),  IVec3::new(1, 1, 0)],
            ],
        ];

        let mut forward_vertices: Vec<ChunkQuadInstance> = vec![];
        let mut backward_vertices: Vec<ChunkQuadInstance> = vec![];

        let u = (d + 1) % 3;
        let v = (d + 2) % 3;
        let mut voxel_pos = IVec3::ZERO;

        let mut block_mask = [(Voxel::Air, 0); CHUNK_SIZE*CHUNK_SIZE];

        voxel_pos[d] = -1;
        while voxel_pos[d] < CHUNK_SIZE as i32 {
            let mut n = 0;
            voxel_pos[v] = 0;

            while voxel_pos[v] < CHUNK_SIZE as i32 {
                voxel_pos[u] = 0;

                while voxel_pos[u] < CHUNK_SIZE as i32 {

                    let block_current = {
                        chunks.get(voxel_pos)
                    };

                    let (block_compare, neigh) = {
                        let mut r = voxel_pos;
                        r[d] += 1;
                        (chunks.get(r), chunks.is_neighbour(r))
                    };

                    // the mask is set to true if there is a visible face
                    // between two blocks, i.e. both aren't empty and both aren't blocks
                    let (voxel, neg_d) = match (block_current.is_transparent(), block_compare.is_transparent()) {
                        (true, false) if !neigh => (block_compare, true),
                        (false, true) => (block_current, false),
                        (_, _) => (Voxel::Air, false),
                    };

                    fn vertex_ao(side1: bool, side2: bool, corner: bool) -> u32 {
                        if side1 && side2 {
                            return 0
                        }

                        return 3 - (side1 as u32 + side2 as u32 + corner as u32)
                    }

                    let mut meta = neg_d as u32;

                    if voxel != Voxel::Air {
                        let mut quad_ao = 0;
                        let index = d + neg_d as usize * 3;
                        let inc = if neg_d { 0 } else { 1 };
                        for (i, offsets) in AO_OFFSETS[index].iter().enumerate() {
                            let mut voxel_pos = voxel_pos;
                            voxel_pos[d] += inc;


                            let side1 = chunks.get(voxel_pos+offsets[0]).is_solid();
                            let side2 = chunks.get(voxel_pos+offsets[1]).is_solid();
                            let corner = chunks.get(voxel_pos+offsets[2]).is_solid();
                            let ao = vertex_ao(side1, side2, corner);

                            quad_ao |= ao << (i*2);
                        }

                        let a00 = quad_ao >> 0 & 0x3;
                        let a01 = quad_ao >> 2 & 0x3;
                        let a10 = quad_ao >> 4 & 0x3;
                        let a11 = quad_ao >> 6 & 0x3;

                        let mut flip = a00 + a11 < a01 + a10;
                        if voxel_pos == IVec3::new(23, 5, 0) && d == 1 {
                            dbg!(a00, a01, a11, a10);
                        }

                        if a00 == 3 && a01 == 2 && a10 == 2 && a11 == 0 {
                            flip = !flip;
                        }
                        else if a00 == 0 && a01 == 2 && a10 == 2 && a11 == 3 {
                            flip = !flip;
                        }
                        else if a00 == 2 && a01 == 0 && a10 == 3 && a11 == 2 {
                            flip = !flip;
                        }
                        else if a00 == 2 && a01 == 3 && a10 == 0 && a11 == 2 {
                            flip = !flip;
                        }

                        quad_ao |= (flip as u32) << 8;
                        meta |= quad_ao << 1;
                    }

                    block_mask[n] = (voxel, meta);
                    n += 1;

                    voxel_pos[u] += 1;
               }

                voxel_pos[v] += 1;
            }


            voxel_pos[d] += 1;


            let mut n = 0;
            for j in 0..CHUNK_SIZE {
                let mut i = 0;
                while i < CHUNK_SIZE {
                    if block_mask[n].0 == Voxel::Air {
                        i += 1;
                        n += 1;
                        continue;
                    }

                    let (kind, meta) = block_mask[n];

                    
                    // Compute the width of this quad and store it in w                        
                    //   This is done by searching along the current axis until mask[n + w] is false
                    let mut w = 1;
                    while i + w < CHUNK_SIZE && block_mask[n + w] == (kind, meta) { w += 1; }


                    // Compute the height of this quad and store it in h                        
                    //   This is done by checking if every block next to this row (range 0 to w) is also part of the mask.
                    //   For example, if w is 5 we currently have a quad of dimensions 1 x 5. To reduce triangle count,
                    //   greedy meshing will attempt to expand this quad out to CHUNK_SIZE x 5, but will stop if it reaches a hole in the mask
                    
                    let mut done = false;
                    let mut h = 1;
                    while j + h < CHUNK_SIZE {
                        for k in 0..w {
                            // if there's a hole in the mask, exit
                            if block_mask[n + k + h * CHUNK_SIZE] != (kind, meta) {
                                done = true;
                                break;
                            }
                        }


                        if done { break }

                        h += 1;
                    }


                    voxel_pos[u] = i as _;
                    voxel_pos[v] = j as _;

                    let neg_d = meta & 0x1;
                    let ao = meta >> 1;

                    if neg_d == 1 {
                        backward_vertices.push(ChunkQuadInstance::new(voxel_pos, kind, h as _, w as _, d as u8 + 3, ao, back_chunk_index));
                    } else {
                        forward_vertices.push(ChunkQuadInstance::new(voxel_pos, kind, h as _, w as _, d as u8, ao, front_chunk_index));
                    }
                    
                    // clear this part of the mask so we don't add duplicates
                    for l in 0..h  {
                        for k in 0..w {
                            block_mask[n+k+l*CHUNK_SIZE].0 = Voxel::Air;
                            block_mask[n+k+l*CHUNK_SIZE].1 = u32::MAX;
                        }
                    }

                    // increment counters and continue
                    i += w;
                    n += w;
                }
            }

        }
        [
            forward_vertices,
            backward_vertices,
        ]
    }
}


/// takes in a world position and returns a chunk position, chunk local position pair
pub fn split_world_pos(pos: IVec3) -> (IVec3, IVec3) {
    let chunk_pos = pos.div_euclid(IVec3::splat(CHUNK_SIZE as i32));
    let chunk_local_pos = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));

    (chunk_pos, chunk_local_pos)
}


/// takes in a chunk position and returns a region position, region local position pair
pub fn split_chunk_pos(pos: IVec3) -> (IVec3, UVec3) {
    let region = pos.div_euclid(IVec3::splat(REGION_SIZE as i32));
    let region_local = pos.rem_euclid(IVec3::splat(REGION_SIZE as i32));
    (region, region_local.as_uvec3())
}
