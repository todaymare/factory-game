pub mod chunk;
pub mod voxel;
pub mod mesh;

use std::{collections::HashMap, fs, hint::spin_loop, ops::Bound, sync::{mpsc, Arc}, time::Instant};

use chunk::{ChunkData, MeshState};
use glam::{DVec3, IVec3, Vec3};
use libnoise::{Perlin, Simplex, Source};
use mesh::{draw_quad, Vertex, VoxelMesh};
use rand::seq::IndexedRandom;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use save_format::byte::{ByteReader, ByteWriter};
use voxel::VoxelKind;

use crate::{directions::Direction, items::{DroppedItem, Item}, perlin::PerlinNoise, quad::Quad, structures::{strct::{InserterState, StructureData}, StructureId, Structures}, voxel_world::{chunk::{Chunk, CHUNK_SIZE}, voxel::Voxel}, PhysicsBody};


type MeshMPSC = (IVec3, Vec<Vertex>, Vec<u32>);
type ChunkMPSC = (IVec3, Chunk);


pub struct VoxelWorld {
    pub chunks: HashMap<IVec3, Option<Chunk>>,
    pub structure_blocks: sti::hash::HashMap<IVec3, StructureId>,
    pub dropped_items: Vec<DroppedItem>,
    remesh_queue: Vec<IVec3>,
    pub unload_queue: Vec<IVec3>,

    perlin_noise: Arc<Perlin<2>>,

    mesh_sender: mpsc::Sender<MeshMPSC>,
    mesh_reciever: mpsc::Receiver<MeshMPSC>,

    chunk_sender: mpsc::Sender<ChunkMPSC>,
    chunk_reciever: mpsc::Receiver<ChunkMPSC>,

    queued_meshes: u32,
    queued_chunks: u32,

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

        Self {
            chunks: HashMap::new(),
            structure_blocks: sti::hash::HashMap::new(),
            dropped_items: vec![],
            remesh_queue: vec![],
            unload_queue: vec![],
            perlin_noise: Arc::new(Source::perlin(6969696969)),
            mesh_sender: ms,
            mesh_reciever: mr,
            chunk_sender: cs,
            chunk_reciever: cr,

            queued_meshes: 0,
            queued_chunks: 0,
        }
    }


    fn spawn_mesh_job(&self, pos: IVec3) {
        let chunks = core::array::from_fn(|i| {
            if i == 0 { Some(self.chunks.get(&pos).unwrap().as_ref().unwrap().data.clone()) }
            else { self.chunks.get(&(pos + SURROUNDING_OFFSETS[i-1])).map(|x| x.as_ref()).flatten().map(|x| x.data.clone()) }
        });

        let sender = self.mesh_sender.clone();

        rayon::spawn(move || {
            let mut vertices = vec![];
            let mut indices = vec![];
            VoxelWorld::remesh_chunk(chunks, &mut vertices, &mut indices);
            sender.send((pos, vertices, indices)).unwrap();
        });
    }




    pub fn process(&mut self) {
        let remesh_queue = core::mem::take(&mut self.remesh_queue);
        for pos in &remesh_queue {
            self.try_get(*pos);
        }

        for pos in &remesh_queue {
            self.ensure_chunk_exists(*pos);
            self.chunks.get_mut(pos).unwrap().as_mut().unwrap().mesh_state = MeshState::Updating;
            self.spawn_mesh_job(*pos);
            self.queued_meshes += 1;
        }

        while let Ok((pos, vertices, indices)) = self.mesh_reciever.try_recv() {
            self.queued_meshes -= 1;
            if vertices.is_empty() {
                continue;
            }

            let mesh = VoxelMesh::new(&vertices, &indices);
            let Some(Some(chunk)) = self.chunks.get_mut(&pos)
            else { continue };
            chunk.mesh = Some(mesh);
            chunk.mesh_state = MeshState::Okay;
        }

        self.process_chunks();

        let mut i = 0;
        while let Some(pos) = self.unload_queue.get(i) {
            let Some(slot) = self.chunks.get(pos)
            else { self.unload_queue.swap_remove(i); continue };

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

        if self.queued_chunks > 0 {
            println!("{} chunks left", self.queued_chunks);
        }
        if self.queued_meshes > 0 {
            println!("{} meshes left", self.queued_meshes);
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
            else { println!("[warn] chunk {pos} was unloaded before being generated"); continue };
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

            for invalidate in SURROUNDING_OFFSETS {
                let pos = pos + invalidate;
                let Some(Some(chunk)) = self.chunks.get_mut(&pos)
                else { continue };
                chunk.mesh_state = MeshState::ShouldUpdate;
                chunk.is_dirty = true;
            }
        }
    }


    pub fn get(&mut self, pos: IVec3) -> Option<&Chunk> {
        if !self.chunks.contains_key(&pos) {
            self.chunks.insert(pos, None);

            let sender = self.chunk_sender.clone();
            let perlin = &*self.perlin_noise.clone();
            let perlin = perlin.clone();
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
            self.chunks.insert(pos, None);

            let sender = self.chunk_sender.clone();
            let perlin = self.perlin_noise.clone();
            self.queued_chunks += 1;
            rayon::spawn(move || {
                let path = format!("saves/chunks/{pos}.chunk");
                let chunk = match fs::read(&path) {
                    Ok(ref v) if let Some(mut byte_reader) = ByteReader::new(&v) => {
                        let mut chunk = Chunk::empty_chunk();
                        let data = Arc::make_mut(&mut chunk.data);

                        for voxel in data.data.iter_mut() {
                            let kind = VoxelKind::from_u8(byte_reader.read_u8().unwrap());
                            voxel.kind = kind;
                        }

                        chunk
                    },


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
        else { return None };

        if chunk.mesh_state == MeshState::ShouldUpdate {
            let chunk = self.chunks.get_mut(&pos).unwrap().as_mut().unwrap();
            chunk.mesh_state = MeshState::Updating;
            self.remesh_queue.push(pos);
        }

        let chunk = self.get_chunk(pos);
        chunk.mesh.as_ref()
    }


    pub fn get_voxel(&mut self, pos: IVec3) -> &Voxel {
        let (chunk_pos, chunk_local_pos) = split_world_pos(pos);

        self.get_chunk(chunk_pos).get(chunk_local_pos)
    }


    pub fn get_voxel_mut(&mut self, pos: IVec3) -> &mut Voxel {
        let (chunk_pos, chunk_local_pos) = split_world_pos(pos);
        self.get_chunk_mut(chunk_pos).get_mut(chunk_local_pos)
    }


    pub fn block_item(&mut self, structures: &Structures, pos: IVec3) -> Item {
        let voxel = self.get_voxel(pos);

        if voxel.kind.is_structure() {
            let structure_id = *self.structure_blocks.get(&pos).unwrap();
            let structure = structures.get(structure_id);
            let kind = structure.data.as_kind().item_kind();
            Item { amount: 1, kind }
        } else {
            let kind = voxel.kind;
            let item = Item { amount: 1, kind: kind.as_item_kind() };
            item
        }

    }


    pub fn break_block(&mut self, structures: &mut Structures, pos: IVec3) -> Item {
        let voxel = self.get_voxel_mut(pos);

        let item = if voxel.kind.is_structure() {
            let structure_id = *self.structure_blocks.get(&pos).unwrap();
            let mut structure = structures.remove(structure_id);
            let placement_origin = structure.position - structure.data.as_kind().origin(structure.direction);
            
            let blocks = structure.data.as_kind().blocks(structure.direction);
            let kind = structure.data.as_kind().item_kind();

            for offset in blocks {
                let pos = placement_origin + offset;

                self.get_voxel_mut(pos).kind = VoxelKind::Air;
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
            let kind = voxel.kind;
            let item = Item { amount: 1, kind: kind.as_item_kind() };
            voxel.kind = VoxelKind::Air;
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

            let is_solid = !voxel.kind.is_air();

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
                        if !self.get_voxel(voxel_pos).kind.is_air() {
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


        while !self.get_voxel(position.floor().as_ivec3()).kind.is_air() {
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
                byte_writer.write_u8(voxel.kind.to_u8());
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
                    let voxel = *chunk.get_usize(x, y, z);

                    if voxel.kind.is_transparent() { continue }

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
                                voxel.kind.is_transparent()
                            } else {
                                false
                            }
                        } else {
                            chunk.get_usize(nx as usize, ny as usize, nz as usize).kind.is_transparent()
                        };

                        if should_draw {
                            draw_quad(vertices, indices,
                                      Quad::from_direction(*dir, voxel_pos, voxel.kind.colour()));
                        }
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
