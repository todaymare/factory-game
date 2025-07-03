pub mod chunk;
pub mod voxel;
pub mod mesh;
pub mod chunker;

use std::{collections::HashMap, fs::{self}, num::NonZeroU32, ops::Bound, sync::{mpsc, Arc}, time::{Duration, Instant}};

use bytemuck::Zeroable;
use chunk::{ChunkData, Noise};
use chunker::{Chunker, MeshTaskData, WorldChunkPos};
use glam::{DVec3, IVec3, UVec3, Vec3, Vec4};
use mesh::{ChunkFaceMesh, ChunkMeshFramedata, ChunkMeshes, ChunkQuadInstance};
use save_format::byte::{ByteReader, ByteWriter};
use sti::key::Key;
use tracing::{info, trace, warn};
use voxel::Voxel;
use wgpu::util::StagingBelt;

use crate::{constants::{CHUNK_SIZE, CHUNK_SIZE_P3, REGION_SIZE}, free_list::FreeKVec, items::{DroppedItem, Item}, octree::MeshOctree, renderer::{gpu_allocator::GPUAllocator, ssbo::SSBO, MeshIndex}, structures::{strct::{InserterState, StructureData}, StructureId, Structures}, voxel_world::chunk::Chunk, PhysicsBody};


type MeshMPSC = (IVec3, [MeshIndex; 6], [(Vec<ChunkQuadInstance>); 6], u32);
type ChunkMPSC = (IVec3, Chunk);
type SaveChunkMPSC = ();


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


    /*
    fn spawn_mesh_job(&mut self, free_list: &mut FreeKVec<MeshIndex, ChunkMeshFramedata>, task_queue: &mut Vec<MeshTaskData>, pos: IVec3) -> bool {
        for offset in SURROUNDING_OFFSETS {
            let Some(Some(_)) = self.try_get_chunk((pos+offset))
            else { assert_ne!(self.queued_chunks, 0); return false; };
        }


        let Some(Some(chunk)) = self.try_get_chunk(pos)
        else {
            assert_ne!(self.queued_chunks, 0); 
            return false;
        };

        if chunk.version.get() == chunk.current_mesh {
            trace!("failed to spawn a mesh job for chunk at '{pos}' because the mesh is up-to-date");
            return true;
        }

        chunk.current_mesh = chunk.version.get();

        if chunk.data.is_none() {
            trace!("failed to spawn a mesh job for chunk at '{pos}' because it's empty");
            return true;
        }

        chunk.is_processing_mesh = true;
        let version = chunk.version;
        let data = chunk.data.clone();

        self.total_meshes += 1;
        self.queued_meshes += 1;


        let offsets = [
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
            free_list.push(ChunkMeshFramedata::zeroed()),
        ];

        let mut chunks : [Option<Arc<ChunkData>>; 7] = [const { None }; 7];
        chunks[0] = data;

        for (i, offset) in SURROUNDING_OFFSETS.iter().enumerate() {
            let Some(Some(chunk)) = self.chunks.get(&(pos+offset))
            else { return false; };

            chunks[i+1] = chunk.data.clone();
        }

        true
    }*/



    pub fn process_meshes(
        &mut self, 
        device: &wgpu::Device, 
        encoder: &mut wgpu::CommandEncoder,
        belt: &mut StagingBelt,
        vertex_allocator: &mut GPUAllocator<ChunkQuadInstance>,
        free_list: &mut FreeKVec<MeshIndex, ChunkMeshFramedata>,
        gpu_mesh_data: &mut SSBO<ChunkMeshFramedata>,
    ) {
        self.chunker.process_mesh_jobs(3, device, encoder, belt, vertex_allocator, free_list, gpu_mesh_data);
        return;

        let now = Instant::now();
        /*
        while now.elapsed().as_millis() < 3
            && let Ok((pos, offsets, result, version)) = self.mesh_reciever.try_recv() {
            self.queued_meshes -= 1;
            let Some(Some(chunk)) = self.chunks.get_mut(&pos)
            else {
                warn!("discarded mesh of chunk '{pos}' because it was unloaded");
                continue;
            };

            //chunk.is_processing_mesh = false;

            /*
            if version < chunk.current_mesh {
                trace!("outdated mesh");
                continue;
            }


            if result.iter().all(|x| x.is_empty()) {
                continue;
            }


            let (region, region_local) = split_chunk_pos(pos);
            let hash = self.mesh_regions.hash(&region);
            let (present, slot) = self.mesh_regions.lookup_for_insert(&region, hash);
            if !present {
                self.mesh_regions.insert_at(slot, hash, region, MeshOctree::new());
            }

            let octree = self.mesh_regions.slot_mut(slot).1;
            if chunk.meshes.is_none() {
                chunk.meshes = Some(octree.insert(region_local, [None, None, None, None, None, None]));
            }

            let meshes = octree.get_mut(chunk.meshes.unwrap());
            for (i, vertices) in result.iter().enumerate() {
                if vertices.is_empty() {
                    meshes[i] = None;
                    free_list.remove(offsets[i]);
                    trace!("discarded mesh of chunk '{pos}' because it was empty");
                    continue;
                }

                let mesh = ChunkFaceMesh::new(belt, encoder, device, vertex_allocator, &vertices, offsets[i]);

                let index = mesh.chunk_mesh_data_index.usize();
                if index >= gpu_mesh_data.len() {
                    warn!("resizing ssbo");
                    gpu_mesh_data.resize(device, encoder, (gpu_mesh_data.len() * 2).max(index+1));
                }

                gpu_mesh_data.write(
                    belt,
                    encoder,
                    device,
                    index,
                    &[ChunkMeshFramedata { offset: pos, normal: i as u32 }]
                );

                self.vertex_count += vertices.len() as u32;
                meshes[i] = Some(mesh);

            }*/
        }

        //println!("remaning meshes: {}, remaining chunks: {}", self.queued_meshes, self.queued_chunks);
        */
    }



    pub fn process(&mut self, voxel_allocator: &mut GPUAllocator<ChunkQuadInstance>, free_list: &mut FreeKVec<MeshIndex, ChunkMeshFramedata>) {

        self.chunker.process_mesh_queue(3, free_list);
        self.chunker.process_chunk_queue(3);
        self.chunker.process_chunk_jobs(3);

        /*
        {
            let mut remesh_queue = core::mem::take(&mut self.remesh_queue);

            let now = Instant::now();
            let mut queue = vec![];
            let mut to_remove = vec![];

            for (_, (&pos, _)) in remesh_queue.iter().enumerate() {
                if now.elapsed().as_millis() > 3 { break }

                let success = self.spawn_mesh_job(free_list, &mut queue, pos);
                if success {
                    let chunk = self.chunks[&pos].as_ref().unwrap();
                    assert_eq!(chunk.version.get(), chunk.current_mesh);
                    to_remove.push(pos);
                }

                if queue.len() == 64 {
                    let sender = self.mesh_sender.clone();
                    rayon::spawn(move || {
                        let time = Instant::now();
                        for item in queue {
                            let mesh = Self::greedy_mesh(item.offsets, item.chunks);
                            if let Err(e) = sender.send((item.pos, item.offsets, mesh, item.version)) {
                                panic!("mesh-generation: {e}");
                            }
                        }
                        trace!("processed 32 meshes in {:?}", time.elapsed());
                    });

                    queue = vec![];
                }

            }

            if queue.len() > 0 {
                let sender = self.mesh_sender.clone();
                rayon::spawn(move || {
                    for item in queue {
                        let mesh = Self::greedy_mesh(item.offsets, item.chunks);
                        if let Err(e) = sender.send((item.pos, item.offsets, mesh, item.version)) {
                            panic!("mesh-generation: {e}");
                        }
                    }
                });

            }

            to_remove.iter().for_each(|p| { remesh_queue.remove(&p); });
            self.remesh_queue = remesh_queue;
        }*/

        //self.process_chunks();

        /*
        let now = Instant::now();
        while now.elapsed().as_millis() < 2000
            && let Some(&(pos, full_unload)) = self.unload_queue.get(0) {

            let Some(slot) = self.chunks.get_mut(&pos)
            else { panic!("tried to unload a chunk that doesn't exist") };

            let Some(chunk) = slot
            else { panic!("tried to unload a chunk before it was fully loaded") };

            if chunk.is_processing_mesh || self.remesh_queue.contains_key(&pos) {
                self.unload_queue.swap_remove(0);
                continue;
            }

            assert!(chunk.is_queued_for_unloading.get());

            'b:{
            if full_unload {
                self.chunk_versions.remove(&pos);
                let (region, region_local) = split_chunk_pos(pos);
                let hash = self.mesh_regions.hash(&region);
                let (present, slot) = self.mesh_regions.lookup_for_insert(&region, hash);
                if !present {
                    break 'b;
                }

                let octree = self.mesh_regions.slot_mut(slot).1;
                //octree.remove(region_local);

                if octree.is_empty() {
                    self.mesh_regions.remove_at(slot);
                }

            } else if chunk.current_mesh == chunk.version.get() {
                self.chunk_versions.insert(pos, chunk.version);
            } else {
                self.chunk_versions.remove(&pos);
            }
            }

            self.unload_queue.swap_remove(0);

            self.save_chunk(pos);
            self.chunks.remove(&pos);

        }*/
    }


    pub fn save_chunk(&mut self, pos: IVec3) {
        /*
        let chunk = self.chunks.get_mut(&pos).unwrap().as_mut().unwrap();
        if !chunk.is_dirty {
            trace!("chunk-save-system: chunk at '{pos}' isn't dirty. skipping saving");
            return;
        }

        chunk.is_dirty = false;

        self.queued_chunk_saves += 1;

        let data = chunk.data.clone();
        let sender = self.save_chunk_sender.clone();

        rayon::spawn(move || {
            let time = Instant::now();
            let mut byte_writer = ByteWriter::new();

            if let Some(data) = data {
                let mut bytes = *data.as_bytes();
                for byte in &mut bytes {
                    if *byte == Voxel::StructureBlock as u8 {
                        *byte = Voxel::Air as u8;
                    }
                }

                byte_writer.write(bytes);
            } else {
                byte_writer.write([Voxel::Air as u8; CHUNK_SIZE_P3]);
            }

            let path = format!("saves/chunks/{pos}.chunk");
            fs::write(path, byte_writer.finish()).unwrap();

            if let Err(_) = sender.send(()) {
                warn!("chunk-save-system: job receiver terminated before all meshing jobs were done");
            }

            info!("chunk-save-system: saved chunk at '{pos}' in {:?}", time.elapsed());

        });*/
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
                    chunk.data = Some(Arc::new(data));
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



    pub fn get_mesh(&self, pos: IVec3) -> Option<&[Option<ChunkFaceMesh>; 6]> {
        /*
        let chunk = self.chunks.get(&pos)?;
        let meshes = chunk.as_ref()?.meshes?;
        let (region, _) = split_chunk_pos(pos);
        let octree = &self.mesh_regions[&region];
        Some(octree.get(meshes))
        */
        todo!()
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
        info!("voxel-save-system: saved the world in {:?}", time.elapsed());
    }



    pub fn greedy_mesh(c: [MeshIndex; 6], chunks: [Option<Arc<ChunkData>>; 7]) -> [Vec<ChunkQuadInstance>; 6]{
        let [west, east] = Self::greedy_mesh_dir(c[0], c[3], &chunks, 0);
        let [up, down] = Self::greedy_mesh_dir(c[1], c[4], &chunks, 1);
        let [north, south] = Self::greedy_mesh_dir(c[2], c[5], &chunks, 2);
        [west, up, north, east, down, south]
    }


    pub fn greedy_mesh_dir(
        front_chunk_index: MeshIndex,
        back_chunk_index: MeshIndex,
        chunks: &[Option<Arc<ChunkData>>; 7],
        d: usize
    ) -> [Vec<ChunkQuadInstance>; 2] {

        let mut forward_vertices: Vec<ChunkQuadInstance> = vec![];
        let mut backward_vertices: Vec<ChunkQuadInstance> = vec![];

        let chunk = &chunks[0];

        let u = (d + 1) % 3;
        let v = (d + 2) % 3;
        let mut x = IVec3::ZERO;

        let mut block_mask = [(Voxel::Air, false); CHUNK_SIZE*CHUNK_SIZE];

        
        let curr_nchunk = match d {
            0 => 2,
            1 => 4,
            2 => 6,
            _ => unreachable!(),
        };
        

        let cmp_nchunk = match d {
            0 => 1,
            1 => 3,
            2 => 5,

            _ => unreachable!(),
        };

        x[d] = -1;
        while x[d] < CHUNK_SIZE as i32 {
            let mut n = 0;
            x[v] = 0;

            while x[v] < CHUNK_SIZE as i32 {
                x[u] = 0;

                while x[u] < CHUNK_SIZE as i32 {

                    let block_current = {
                        let r = x;
                        let is_out_of_bounds =    r.x < 0
                                               || r.y < 0
                                               || r.z < 0;

                        if is_out_of_bounds {
                            let nchunk = &chunks[curr_nchunk];
                            let pos = r;
                            let voxel = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));
                            nchunk.as_ref().map(|c| c.get(voxel)).unwrap_or(Voxel::Air)
                        } else {
                            chunk.as_ref().map(|c| c.get(r)).unwrap_or(Voxel::Air)
                        }
                    };

                    let block_compare = {
                        let mut r = x;
                        r[d] += 1;
                        let is_out_of_bounds =    r.x == CHUNK_SIZE as i32
                                               || r.y == CHUNK_SIZE as i32
                                               || r.z == CHUNK_SIZE as i32;

                        if is_out_of_bounds {
                            let nchunk = &chunks[cmp_nchunk];
                            let pos = r;
                            let voxel = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));
                            nchunk.as_ref().map(|c| c.get(voxel)).unwrap_or(Voxel::Air)
                        } else {
                            chunk.as_ref().map(|c| c.get(r)).unwrap_or(Voxel::Air)
                        }
                    };

                    // the mask is set to true if there is a visible face
                    // between two blocks, i.e. both aren't empty and both aren't blocks
                    block_mask[n] = match (block_current.is_transparent(), block_compare.is_transparent()) {
                        (true, false) => (block_compare, true),
                        (false, true) => (block_current, false),
                        (_, _) => (Voxel::Air, false),
                    };
                    n += 1;

                    x[u] += 1;
                }

                x[v] += 1;
            }


            x[d] += 1;


            let mut n = 0;
            for j in 0..CHUNK_SIZE {
                let mut i = 0;
                while i < CHUNK_SIZE {
                    if block_mask[n].0 == Voxel::Air {
                        i += 1;
                        n += 1;
                        continue;
                    }

                    let (kind, neg_d) = block_mask[n];

                    
                    // Compute the width of this quad and store it in w                        
                    //   This is done by searching along the current axis until mask[n + w] is false
                    let mut w = 1;
                    while i + w < CHUNK_SIZE && block_mask[n + w] == (kind, neg_d) { w += 1; }


                    // Compute the height of this quad and store it in h                        
                    //   This is done by checking if every block next to this row (range 0 to w) is also part of the mask.
                    //   For example, if w is 5 we currently have a quad of dimensions 1 x 5. To reduce triangle count,
                    //   greedy meshing will attempt to expand this quad out to CHUNK_SIZE x 5, but will stop if it reaches a hole in the mask
                    
                    let mut done = false;
                    let mut h = 1;
                    while j + h < CHUNK_SIZE {
                        for k in 0..w {
                            // if there's a hole in the mask, exit
                            if block_mask[n + k + h * CHUNK_SIZE] != (kind, neg_d) {
                                done = true;
                                break;
                            }
                        }


                        if done { break }

                        h += 1;
                    }


                    x[u] = i as _;
                    x[v] = j as _;

                    if neg_d {
                        backward_vertices.push(ChunkQuadInstance::new(x, kind.colour(), h as _, w as _, d as u8 + 3, back_chunk_index));
                    } else {
                        forward_vertices.push(ChunkQuadInstance::new(x, kind.colour(), h as _, w as _, d as u8, front_chunk_index));
                    }
                    
                    // clear this part of the mask so we don't add duplicates
                    for l in 0..h  {
                        for k in 0..w {
                            block_mask[n+k+l*CHUNK_SIZE].0 = Voxel::Air;
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
