pub mod chunk;
pub mod voxel;
pub mod mesh;

use std::{collections::{HashMap, HashSet}, fs::{self, File}, hint::spin_loop, io::Write, ops::Bound, sync::{mpsc, Arc}, time::{Duration, Instant}};

use chunk::{ChunkData, MeshState, Noise};
use glam::{DVec3, IVec3, Vec3, Vec4};
use libnoise::{Perlin, Simplex, Source};
use mesh::{draw_quad, Vertex, VoxelMesh};
use rand::seq::IndexedRandom;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use save_format::byte::{ByteReader, ByteWriter};
use tracing::{info, warn};
use voxel::Voxel;

use crate::{directions::Direction, items::{DroppedItem, Item}, mesh::Mesh, perlin::PerlinNoise, quad::Quad, structures::{strct::{InserterState, StructureData}, StructureId, Structures}, voxel_world::chunk::{Chunk, CHUNK_SIZE}, PhysicsBody};


type MeshMPSC = (IVec3, Vec<Vertex>, Vec<u32>, Duration);
type ChunkMPSC = (IVec3, Chunk);


pub struct VoxelWorld {
    pub chunks: HashMap<IVec3, Option<Chunk>>,
    pub structure_blocks: sti::hash::HashMap<IVec3, StructureId>,
    pub dropped_items: Vec<DroppedItem>,
    remesh_queue: HashSet<IVec3>,
    pub unload_queue: Vec<IVec3>,

    pub noise: Arc<Noise>,

    mesh_sender: mpsc::Sender<MeshMPSC>,
    mesh_reciever: mpsc::Receiver<MeshMPSC>,

    chunk_sender: mpsc::Sender<ChunkMPSC>,
    chunk_reciever: mpsc::Receiver<ChunkMPSC>,

    queued_meshes: u32,
    queued_chunks: u32,
    total_meshes: u32,
    total_chunks: u32,

    pub loading_chunk_mesh: VoxelMesh,
    timings: Vec<Duration>,

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

        let mut full_chunk = Chunk::empty_chunk();
        let data = Arc::make_mut(&mut full_chunk.data);
        data.data.iter_mut().for_each(|x| *x = Voxel::Stone);

        let mut vertices = vec![];
        let mut indices = vec![];

        let arr = core::array::from_fn(|i| if i == 0 { full_chunk.data.clone() } else { Chunk::empty_chunk().data.clone() });
        Self::greedy_mesh(arr, &mut vertices, &mut indices);

        for vertex in &mut vertices {
            vertex.set_colour(Vec4::new(1.0, 0.0, 0.0, 1.0));
        }

        let mesh = VoxelMesh::new(&vertices, &indices);

        Self {
            total_meshes: 0,
            total_chunks: 0,
            timings: vec![],
            chunks: HashMap::new(),
            structure_blocks: sti::hash::HashMap::new(),
            dropped_items: vec![],
            remesh_queue: HashSet::new(),
            unload_queue: vec![],
            noise: Arc::new(Noise::new(6969696969)),
            mesh_sender: ms,
            mesh_reciever: mr,
            chunk_sender: cs,
            chunk_reciever: cr,

            queued_meshes: 0,
            queued_chunks: 0,

            loading_chunk_mesh: mesh,
        }

    }


    fn spawn_mesh_job(&mut self, pos: IVec3) {
        for i in 0..7 {
            let chunk_pos = if i == 0 { pos }
                      else { pos + SURROUNDING_OFFSETS[i-1] };

            if self.try_get(chunk_pos).is_none() {
                self.remesh_queue.insert(pos);
                return;
            }
        }

        let chunk = self.chunks.get_mut(&pos).unwrap().as_mut().unwrap();
        if chunk.mesh_state != MeshState::ShouldUpdate {
            return;
        }

        if chunk.data.is_empty() {
            return;
        }

        chunk.mesh_state = MeshState::Updating;

        self.total_meshes += 1;
        self.queued_meshes += 1;

        let chunks = core::array::from_fn(|i| {
            let pos = if i == 0 { pos }
                      else { pos + SURROUNDING_OFFSETS[i-1] };

            self.chunks[&pos].as_ref().unwrap().data.clone()
        });

        let sender = self.mesh_sender.clone();

        rayon::spawn(move || {
            let mut vertices = vec![];
            let mut indices = vec![];
            let time = Instant::now();
            VoxelWorld::greedy_mesh(chunks, &mut vertices, &mut indices);
            if let Err(_) = sender.send((pos, vertices, indices, time.elapsed())) {
                warn!("job receiver terminated before all meshing jobs were done");
            }
        });
    }




    pub fn process(&mut self) {
        let remesh_queue = core::mem::take(&mut self.remesh_queue);

        for pos in &remesh_queue {
            self.spawn_mesh_job(*pos);
        }

        while let Ok((pos, vertices, indices, dur)) = self.mesh_reciever.try_recv() {
            self.timings.push(dur);
            self.queued_meshes -= 1;
            let Some(Some(chunk)) = self.chunks.get_mut(&pos)
            else {
                info!("discarded mesh of chunk '{pos}' because it was unloaded");
                continue;
            };

            chunk.mesh_state = MeshState::Okay;

            if vertices.is_empty() {
                continue;
            }

            let mesh = VoxelMesh::new(&vertices, &indices);
            chunk.mesh = Some(mesh);
        }

        if self.queued_meshes == 0 && self.queued_chunks == 0 {
        } else {
            println!("{} total meshes {} total chunks {} meshes left {} chunks left", self.total_meshes, self.total_chunks, self.queued_meshes, self.queued_chunks);
        }


        self.process_chunks();

        let mut i = 0;
        while let Some(pos) = self.unload_queue.get(i) {
            let Some(slot) = self.chunks.get(pos)
            else { i += 1; continue };

            let Some(chunk) = slot
            else { i += 1; continue; };

            if chunk.persistent {
                self.unload_queue.swap_remove(i);
                continue;
            }

            if chunk.mesh_state == MeshState::Updating {
                i += 1;
                continue;
            }

            self.chunks.remove(pos);
            self.unload_queue.remove(i);
        }
    }


    pub fn get_chunk(&mut self, pos: IVec3) -> &Chunk {
        self.ensure_chunk_exists(pos);
        self.chunks.get(&pos).unwrap().as_ref().unwrap()
    }


    pub fn get_chunk_mut(&mut self, pos: IVec3) -> &mut Chunk {
        self.ensure_chunk_exists(pos);

        for invalidate in SURROUNDING_OFFSETS {
            let pos = pos + invalidate;
            let Some(Some(chunk)) = self.chunks.get_mut(&pos)
            else { continue };
            chunk.mesh_state = MeshState::ShouldUpdate;
            chunk.is_dirty = true;
        }


        let chunk = self.chunks.get_mut(&pos).unwrap().as_mut().unwrap();
        chunk.mesh_state = MeshState::ShouldUpdate;
        chunk.is_dirty = true;

        chunk
    }


    pub fn ensure_chunk_exists(&mut self, pos: IVec3) {
        // this will queue the job worst-case
        if self.try_get(pos).is_some() { return }

        // then we process until there's nothing left
        // by which point we should be good to go
        while self.chunks[&pos].is_none() {
            self.process_chunks();
        }

        debug_assert!(self.try_get(pos).is_some());
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
                chunk.mesh_state = MeshState::ShouldUpdate;
                chunk.is_dirty = true;
            }


        }
    }


    pub fn process_blocking(&mut self) {
        for (pos, chunk) in self.chunk_reciever.iter() {
            let slot = self.chunks.get_mut(&pos).unwrap();
            *slot = Some(chunk);
        }
    }


    pub fn get(&mut self, pos: IVec3) -> Option<&Chunk> {
        if !self.chunks.contains_key(&pos) {
            self.chunks.insert(pos, None);

            let sender = self.chunk_sender.clone();
            let perlin = self.noise.clone();
            rayon::spawn(move || {
                let chunk = Chunk::generate(pos, &perlin);
                sender.send((pos, chunk)).unwrap();
            });

            return None;
        }
        self.chunks[&pos].as_ref()
    }


    pub fn try_get(&mut self, pos: IVec3) -> Option<&Chunk> {
        if !self.chunks.contains_key(&pos) {
            self.total_chunks += 1;
            self.chunks.insert(pos, None);

            let sender = self.chunk_sender.clone();
            let perlin = self.noise.clone();
            self.queued_chunks += 1;
            rayon::spawn(move || {
                let path = format!("saves/chunks/{pos}.chunk");
                let chunk = match fs::read(&path) {
                    /*
                    Ok(ref v) if let Some(mut byte_reader) = ByteReader::new(&v) => {
                        let mut chunk = Chunk::empty_chunk();
                        let data = Arc::make_mut(&mut chunk.data);

                        for voxel in data.data.iter_mut() {
                            let kind = VoxelKind::from_u8(byte_reader.read_u8().unwrap());
                            voxel.kind = kind;
                        }

                        chunk
                    },*/


                    _ => {
                        Chunk::generate(pos, &perlin)
                    }
                };
                match sender.send((pos, chunk)) {
                    Ok(_) => (),
                    Err(e) => panic!("{}", e.to_string()),
                }
            });

            return None;
        }
        self.chunks[&pos].as_ref()
    }


    pub fn try_get_mesh(&mut self, pos: IVec3) -> Option<&VoxelMesh> {
        let Some(chunk) = self.try_get(pos)
        else { return Some(&self.loading_chunk_mesh) };

        if chunk.mesh_state == MeshState::ShouldUpdate {
            self.remesh_queue.insert(pos);
        }

        let chunk = self.get_chunk(pos);
        chunk.mesh.as_ref()
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


    pub fn break_block(&mut self, structures: &mut Structures, pos: IVec3) -> Item {
        let voxel = self.get_voxel_mut(pos);

        let item = if voxel.is_structure() {
            let structure_id = *self.structure_blocks.get(&pos).unwrap();
            let mut structure = structures.remove(structure_id);
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


            for index in 0..structure.available_items_len() {
                while let Some(item) = structure.try_take(index) {
                    self.dropped_items.push(DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)));
                }
            }


            match structure.data {
                StructureData::Inserter { state: InserterState::Placing(item), .. } => {
                    self.dropped_items.push(DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)));
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
        for (&pos, chunk) in &mut self.chunks {
            let Some(chunk) = chunk.as_mut()
            else { continue };
            if !chunk.is_dirty { continue }
            chunk.is_dirty = false;
            let mut byte_writer = ByteWriter::new();

            for voxel in &chunk.data.data {
                byte_writer.write_u8(voxel.to_u8());
            }

            let path = format!("saves/chunks/{pos}.chunk");
            fs::write(path, byte_writer.finish()).unwrap();
        }
    }


    pub fn remesh_chunk(chunks: [Option<Arc<ChunkData>>; 7], vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) -> bool {
        // direction of the face
        // the block offset to move in that direction
        // which chunk data to use if the block in that direction isn't in here
        const FACE_DIRECTIONS: [(Direction, (i32, i32, i32), u32); 6] = [
            (Direction::Right,   (-1,  0,  0), 2),
            (Direction::Left,    ( 1,  0,  0), 1),
            (Direction::Up,      ( 0,  1,  0), 3),
            (Direction::Down,    ( 0, -1,  0), 4),
            (Direction::Forward, ( 0,  0,  1), 5),
            (Direction::Back,    ( 0,  0, -1), 6),
        ];

        let chunk = chunks[0].as_ref().unwrap();

        for z in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let voxel = chunk.get_usize(x, y, z);

                    if voxel.is_transparent() { continue }

                    let voxel_pos = Vec3::new(x as f32, y as f32, z as f32);

                    for (dir, (dx, dy, dz), nchunk) in FACE_DIRECTIONS.iter() {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        let nz = z as i32 + dz;

                        let is_out_of_bounds = nx < 0 || nx >= CHUNK_SIZE as i32
                                            || ny < 0 || ny >= CHUNK_SIZE as i32
                                            || nz < 0 || nz >= CHUNK_SIZE as i32;

                        let should_draw = if is_out_of_bounds {
                            let nchunk = &chunks[*nchunk as usize];
                            if let Some(nchunk) = nchunk {
                                let pos = IVec3::new(nx, ny, nz);
                                let voxel = (pos + 32) % 32;
                                let voxel = voxel.as_usizevec3();
                                let voxel = nchunk.get_usize(voxel.x, voxel.y, voxel.z);
                                voxel.is_transparent()
                            } else {
                                false
                            }
                        } else {
                            chunk.get_usize(nx as usize, ny as usize, nz as usize).is_transparent()
                        };

                        if should_draw {
                            mesh::draw_quad(vertices, indices,
                                      mesh::Quad::from_direction(*dir, voxel_pos, voxel.colour()));
                        }
                    }
                }
            }
        }

        true
    }


    pub fn greedy_mesh(chunks: [Arc<ChunkData>; 7], vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) -> bool {
        let chunk = &chunks[0];
        // sweep over each axis

        for d in 0..3 {
            let u = (d + 1) % 3;
            let v = (d + 2) % 3;
            let mut x = IVec3::ZERO;
            let mut q = IVec3::ZERO;

            let mut block_mask = [Voxel::Air; CHUNK_SIZE*CHUNK_SIZE];
            q[d] = 1;

            x[d] = 0;
            while x[d] < CHUNK_SIZE as i32 {
                let mut n = 0;
                x[v] = 0;

                while x[v] < CHUNK_SIZE as i32 {
                    x[u] = 0;

                    while x[u] < CHUNK_SIZE as i32 {

                        let block_current = chunk.get(x);


                        let block_compare = if x[d] < CHUNK_SIZE as i32 {
                            let r = x+q;
                            let is_out_of_bounds =    r.x < 0 || r.x >= CHUNK_SIZE as i32
                                                   || r.y < 0 || r.y >= CHUNK_SIZE as i32
                                                   || r.z < 0 || r.z >= CHUNK_SIZE as i32;

                            if is_out_of_bounds {
                                let nchunk = match q.to_array() {
                                    [ 1,  0,  0] => 1,
                                    [-1,  0,  0] => 2,
                                    [ 0,  1,  0] => 3,
                                    [ 0, -1,  0] => 4,
                                    [ 0,  0,  1] => 5,
                                    [ 0,  0, -1] => 6,
                                    _ => unreachable!(),
                                };


                                let nchunk = &chunks[nchunk];
                                let pos = r;
                                let voxel = (pos + CHUNK_SIZE as i32) % CHUNK_SIZE as i32;
                                nchunk.get(voxel)
                            } else {
                                chunk.get(r)
                            }
                        } else { block_current };

                        // the mask is set to true if there is a visible face
                        // between two blocks, i.e. both aren't empty and both aren't blocks
                        block_mask[n] = match (block_current, block_compare) {
                            (Voxel::Air, Voxel::Air) => Voxel::Air,
                            (Voxel::Air, other) => other,
                            (current, Voxel::Air) => current,
                            (_, _) => Voxel::Air,
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
                        if block_mask[n] == Voxel::Air {
                            i += 1;
                            n += 1;
                            continue;
                        }

                        let kind = block_mask[n];

                        
                        // Compute the width of this quad and store it in w                        
                        //   This is done by searching along the current axis until mask[n + w] is false
                        let mut w = 1;
                        while i + w < CHUNK_SIZE && block_mask[n + w] == kind { w += 1; }


                        // Compute the height of this quad and store it in h                        
                        //   This is done by checking if every block next to this row (range 0 to w) is also part of the mask.
                        //   For example, if w is 5 we currently have a quad of dimensions 1 x 5. To reduce triangle count,
                        //   greedy meshing will attempt to expand this quad out to CHUNK_SIZE x 5, but will stop if it reaches a hole in the mask
                        
                        let mut done = false;
                        let mut h = 1;
                        while j + h < CHUNK_SIZE {
                            for k in 0..w {
                                // if there's a hole in the mask, exit
                                if block_mask[n + k + h * CHUNK_SIZE] != kind {
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

                        let quad = mesh::Quad {
                            color: kind.colour(),
                            corners: [
                                x.as_vec3(),
                                (x+du).as_vec3(),
                                (x+du+dv).as_vec3(),
                                (x+dv).as_vec3(),
                            ],
                        };

                        draw_quad(vertices, indices, quad);


                        // clear this part of the mask so we don't add duplicates
                        for l in 0..h  {
                            for k in 0..w {
                                block_mask[n+k+l*CHUNK_SIZE] = Voxel::Air;
                            }
                        }

                        // increment counters and continue
                        i += w;
                        n += w;
                    }
                }

            }
        }
        true
    }
}


pub fn split_world_pos(pos: IVec3) -> (IVec3, IVec3) {
    let chunk_pos = pos.div_euclid(IVec3::splat(CHUNK_SIZE as i32));
    let chunk_local_pos = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));

    (chunk_pos, chunk_local_pos)
}
