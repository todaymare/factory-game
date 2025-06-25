pub mod chunk;
pub mod voxel;
pub mod mesh;

use std::{collections::{HashMap, HashSet}, fs::{self}, ops::Bound, sync::{mpsc, Arc}, time::Instant};

use chunk::{ChunkData, Noise};
use glam::{DVec3, IVec3, Vec3, Vec4};
use mesh::{draw_quad, Vertex, ChunkMesh};
use rand::seq::IndexedRandom;
use save_format::byte::{ByteReader, ByteWriter};
use tracing::{error, info, trace, warn};
use voxel::Voxel;
use voxel_mesher::VoxelMesh;

use crate::{constants::CHUNK_SIZE, items::{DroppedItem, Item}, structures::{strct::{InserterState, StructureData}, StructureId, Structures}, voxel_world::chunk::Chunk, PhysicsBody};


type MeshMPSC = (IVec3, [(Vec<Vertex>, Vec<u32>); 6], u32);
type ChunkMPSC = (IVec3, Chunk);
type SaveChunkMPSC = ();


pub struct VoxelWorld {
    pub chunks: sti::hash::HashMap<IVec3, Option<Chunk>>,
    pub structure_blocks: sti::hash::HashMap<IVec3, StructureId>,
    pub dropped_items: Vec<DroppedItem>,
    remesh_queue: HashSet<IVec3>,
    pub unload_queue: Vec<IVec3>,

    pub noise: Arc<Noise>,

    mesh_sender: mpsc::Sender<MeshMPSC>,
    mesh_reciever: mpsc::Receiver<MeshMPSC>,

    chunk_sender: mpsc::Sender<ChunkMPSC>,
    chunk_reciever: mpsc::Receiver<ChunkMPSC>,

    save_chunk_sender: mpsc::Sender<SaveChunkMPSC>,
    save_chunk_receiver: mpsc::Receiver<SaveChunkMPSC>,

    queued_meshes: u32,
    queued_chunks: u32,
    queued_chunk_saves: u32,

    total_meshes: u32,
    total_chunks: u32,


    indicies: Vec<u32>,
}


const SURROUNDING_OFFSETS : &[IVec3] = &[
    IVec3::new( 1,  0,  0),
    IVec3::new(-1,  0,  0),
    IVec3::new( 0,  1,  0),
    IVec3::new( 0, -1,  0),
    IVec3::new( 0,  0,  1),
    IVec3::new( 0,  0, -1),
];


impl VoxelWorld {
    pub fn new() -> Self {
        let (ms, mr) = mpsc::channel();
        let (cs, cr) = mpsc::channel();
        let (scs, scr) = mpsc::channel();

        let mut full_chunk = Chunk::empty_chunk();
        let data = Arc::make_mut(&mut full_chunk.data);
        data.data.iter_mut().for_each(|x| *x = Voxel::Stone);

        Self {
            total_meshes: 0,
            total_chunks: 0,

            chunks: sti::hash::HashMap::new(),
            structure_blocks: sti::hash::HashMap::new(),
            dropped_items: vec![],
            remesh_queue: HashSet::new(),
            unload_queue: vec![],
            noise: Arc::new(Noise::new(6969696969)),
            mesh_sender: ms,
            mesh_reciever: mr,
            chunk_sender: cs,
            chunk_reciever: cr,
            save_chunk_sender: scs,
            save_chunk_receiver: scr,

            queued_meshes: 0,
            queued_chunks: 0,
            queued_chunk_saves: 0,

            indicies: vec![],
        }

    }


    fn spawn_mesh_job(&mut self, pos: IVec3) {
        for i in 0..7 {
            let chunk_pos = if i == 0 { pos }
                      else { pos + SURROUNDING_OFFSETS[i-1] };

            if self.try_get_chunk(chunk_pos).is_none() {
                self.remesh_queue.insert(pos);
                return;
            }
        }

        let chunk = self.chunks.get_mut(&pos).unwrap().as_mut().unwrap();
        if chunk.version == chunk.current_mesh {
            trace!("failed to spawn a mesh job for chunk at '{pos}' because the mesh is fine");
            return;
        }

        if chunk.data.is_empty() {
            trace!("failed to spawn a mesh job for chunk at '{pos}' because it's empty");
            chunk.current_mesh = chunk.version;
            return;
        }

        self.total_meshes += 1;
        self.queued_meshes += 1;
        let version = chunk.version;
        chunk.current_mesh = chunk.version;

        let chunks = core::array::from_fn(|i| {
            let pos = if i == 0 { pos }
                      else { pos + SURROUNDING_OFFSETS[i-1] };

            self.chunks[&pos].as_ref().unwrap().data.clone()
        });

        let sender = self.mesh_sender.clone();

        rayon::spawn(move || {
            let time = Instant::now();
            let result = VoxelWorld::greedy_mesh(chunks);
            trace!("mesh-generation: meshes in {:?}", time.elapsed());
            if let Err(_) = sender.send((pos, result, version)) {
                warn!("mesh-generation: job receiver terminated before all meshing jobs were done");
            }
        });
    }




    pub fn process(&mut self) {
        let remesh_queue = core::mem::take(&mut self.remesh_queue);

        for pos in &remesh_queue {
            self.spawn_mesh_job(*pos);
        }

        while let Ok((pos, result, version)) = self.mesh_reciever.try_recv() {
            self.queued_meshes -= 1;
            let Some(Some(chunk)) = self.chunks.get_mut(&pos)
            else {
                info!("discarded mesh of chunk '{pos}' because it was unloaded");
                continue;
            };

            if version < chunk.current_mesh {
                info!("outdated mesh");
                continue;
            }

            for (i, (vertices, indices)) in result.iter().enumerate() {
                self.indicies.push(indices.len() as u32);

                if vertices.is_empty() {
                    chunk.meshes[i] = None;
                    info!("discarded mesh of chunk '{pos}' because it was empty");
                    continue;
                }

                let mesh = ChunkMesh::new(&vertices, &indices);
                chunk.meshes[i] = Some(mesh);
            }

        }

        if self.queued_meshes == 0 && self.queued_chunks == 0 {
        } else {
            info!("{} total meshes {} total chunks {} meshes left {} chunks left", self.total_meshes, self.total_chunks, self.queued_meshes, self.queued_chunks);
        }


        self.process_chunks();

        let mut i = 0;
        while let Some(&pos) = self.unload_queue.get(i) {
            let Some(slot) = self.chunks.get(&pos)
            else { i += 1; continue };

            let Some(chunk) = slot
            else { i += 1; continue; };

            if chunk.persistent {
                self.unload_queue.swap_remove(i);
                continue;
            }

            self.unload_queue.remove(i);
            self.save_chunk(pos);

            self.chunks.remove(&pos);
        }
    }


    pub fn save_chunk(&mut self, pos: IVec3) {
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

            let mut bytes = *data.as_bytes();
            for byte in &mut bytes {
                if *byte == Voxel::StructureBlock as u8 {
                    *byte = Voxel::Air as u8;
                }
            }
            byte_writer.write(bytes);

            let path = format!("saves/chunks/{pos}.chunk");
            fs::write(path, byte_writer.finish()).unwrap();

            if let Err(_) = sender.send(()) {
                warn!("chunk-save-system: job receiver terminated before all meshing jobs were done");
            }

            info!("chunk-save-system: saved chunk at '{pos}' in {:?}", time.elapsed());

        });
    }


    pub fn get_chunk(&mut self, pos: IVec3) -> &Chunk {
        &*self.ensure_chunk_exists(pos)
    }


    pub fn get_chunk_mut(&mut self, pos: IVec3) -> &mut Chunk {
        for invalidate in SURROUNDING_OFFSETS {
            let pos = pos + invalidate;
            let Some(Some(chunk)) = self.chunks.get_mut(&pos)
            else { continue };

            chunk.version += 1;
        }


        let chunk = self.ensure_chunk_exists(pos);
        chunk.version += 1;
        chunk.is_dirty = true;

        chunk
    }


    pub fn try_get_chunk(&mut self, pos: IVec3) -> Option<&Chunk> {
        let hash = self.chunks.hash(&pos);
        let (present, slot) = self.chunks.lookup_for_insert(&pos, hash);
        if !present {
            self.chunks.insert_at(slot, hash, pos, None);
            self.total_chunks += 1;
            self.chunks.insert(pos, None);

            let sender = self.chunk_sender.clone();
            let perlin = self.noise.clone();
            self.queued_chunks += 1;
            rayon::spawn(move || {
                let chunk = Self::chunk_creation_job(pos, &perlin);

                if let Err(_) = sender.send((pos, chunk)) {
                    warn!("chunk-generation-system: job receiver terminated before all meshing jobs were done");
                }
            });

            return None;
        }
        return self.chunks.slot(slot).1.as_ref();
    }


    pub fn ensure_chunk_exists(&mut self, pos: IVec3) -> &mut Chunk {
        let hash = self.chunks.hash(&pos);
        let (present, slot) = self.chunks.lookup_for_insert(&pos, hash);

        if !present || self.chunks.slot(slot).1.is_none() {
            let chunk = Self::chunk_creation_job(pos, &self.noise);
            self.chunks.insert_at(slot, hash, pos, Some(chunk));
        }

        return self.chunks.slot_mut(slot).1.as_mut().unwrap();
    }


    pub fn process_chunks(&mut self) {
        while let Ok((pos, chunk)) = self.chunk_reciever.try_recv() {
            self.queued_chunks -= 1;

            let Some(slot) = self.chunks.get_mut(&pos)
            else { warn!("chunk {pos} was unloaded before being generated"); continue };
            *slot = Some(chunk);


            for invalidate in SURROUNDING_OFFSETS {
                let pos = pos + invalidate;
                let Some(Some(chunk)) = self.chunks.get_mut(&pos)
                else { continue };
                chunk.version += 1;
            }


        }
    }


    pub fn process_blocking(&mut self) {
        trace!("processing chunks, blocking");
        while self.queued_chunks > 0 {
            let Ok((pos, chunk)) = self.chunk_reciever.try_recv()
            else { continue };

            self.queued_chunks -= 1;

            let Some(slot) = self.chunks.get_mut(&pos)
            else { warn!("chunk {pos} was unloaded before being generated"); continue };
            *slot = Some(chunk);
        }

        trace!("all chunk generation is complete");
    }



    pub fn chunk_creation_job(pos: IVec3, noise: &Noise) -> Chunk {
        let path = format!("saves/chunks/{pos}.chunk");
        let chunk = match fs::read(&path) {
            Ok(ref v) if let Some(mut byte_reader) = ByteReader::new(&v) => {
                let mut chunk = Chunk::empty_chunk();
                let data = Arc::make_mut(&mut chunk.data);

                *data = ChunkData::from_bytes(byte_reader.read().unwrap());

                chunk.is_dirty = false;
                chunk
            },


            _ => {
                Chunk::generate(pos, noise)
            }
        };

        chunk

    }


    pub fn try_get_mesh(&mut self, pos: IVec3) -> Option<&[Option<ChunkMesh>; 6]> {
        let chunk = self.try_get_chunk(pos)?;

        if chunk.version != chunk.current_mesh {
            info!("queueing the chunk at '{pos}' for remeshing");
            self.remesh_queue.insert(pos);
        }

        let chunk = self.get_chunk(pos);
        Some(&chunk.meshes)
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
        trace!("voxel-save-system: saving the world..");
        let time = Instant::now();
        self.process_blocking();

        // we just need the jobs to be over so we don't spam warnings
        while self.queued_meshes > 0 { if self.mesh_reciever.try_recv().is_ok() { self.queued_meshes -= 1} };

        for (pos, _) in self.chunks.iter() {
            self.unload_queue.push(*pos);
        }

        // unload all chunks
        let save_queue = core::mem::take(&mut self.unload_queue);
        for pos in save_queue {
            self.save_chunk(pos);
        }


        while self.queued_chunk_saves > 0 { if self.save_chunk_receiver.try_recv().is_ok() { self.queued_chunk_saves -= 1} };
        info!("voxel-save-system: saved the world in {:?}", time.elapsed());

        //error!("average indicies: {}", self.indicies.iter().sum::<u32>() as usize / self.indicies.len());
    }



    pub fn greedy_mesh(chunks: [Arc<ChunkData>; 7]) -> [(Vec<Vertex>, Vec<u32>); 6]{
        let [west, east] = Self::greedy_mesh_dir(&chunks, 0);
        let [up, down] = Self::greedy_mesh_dir(&chunks, 1);
        let [north, south] = Self::greedy_mesh_dir(&chunks, 2);
        [west, up, north, east, down, south]
    }


    pub fn greedy_mesh_dir(chunks: &[Arc<ChunkData>; 7],
                           d: usize,) -> [(Vec<Vertex>, Vec<u32>); 2] {

        let mut forward_vertices: Vec<Vertex> = vec![];
        let mut forward_indices: Vec<u32> = vec![];
        let mut backward_vertices: Vec<Vertex> = vec![];
        let mut backward_indices: Vec<u32> = vec![];

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
                            nchunk.get(voxel)
                        } else {
                            chunk.get(r)
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
                            nchunk.get(voxel)
                        } else {
                            chunk.get(r)
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

                    // du and dv determine the size and orientation of this face
                    let mut du = IVec3::ZERO;
                    du[u] = w as _;

                    let mut dv = IVec3::ZERO;
                    dv[v] = h as _;

                    if neg_d {
                        let quad =  mesh::Quad {
                                    //color: if neg_d { Vec4::new(1.0, 0.0, 0.0, 1.0) }
                                    //       else { Vec4::new(0.0, 1.0, 0.0, 1.0) },
                                    color: kind.colour(),
                                    corners: [
                                        (x+dv).as_vec3(),
                                        (x+du+dv).as_vec3(),
                                        (x+du).as_vec3(),
                                        x.as_vec3(),
                                    ],
                                    normal: d as u8 + 3,
                                };

                        draw_quad(&mut backward_vertices, &mut backward_indices, quad);
                    } else {
                        let quad =  mesh::Quad {
                                    //color: if neg_d { Vec4::new(1.0, 0.0, 0.0, 1.0) }
                                    //       else { Vec4::new(0.0, 1.0, 0.0, 1.0) },
                                    color: kind.colour(),
                                    corners: [
                                        x.as_vec3(),
                                        (x+du).as_vec3(),
                                        (x+du+dv).as_vec3(),
                                        (x+dv).as_vec3(),
                                    ],
                                    normal: d as u8,
                                };

                        draw_quad(&mut forward_vertices, &mut forward_indices, quad);
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
            (forward_vertices, forward_indices),
            (backward_vertices, backward_indices),
        ]
    }
}


pub fn split_world_pos(pos: IVec3) -> (IVec3, IVec3) {
    let chunk_pos = pos.div_euclid(IVec3::splat(CHUNK_SIZE as i32));
    let chunk_local_pos = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));

    (chunk_pos, chunk_local_pos)
}



