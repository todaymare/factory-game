pub mod chunk;
pub mod voxel;

use std::{collections::HashMap, fs, ops::Bound, time::Instant};

use chunk::MeshState;
use glam::{IVec3, Vec3};
use save_format::byte::{ByteReader, ByteWriter};
use voxel::VoxelKind;

use crate::{directions::Direction, items::{DroppedItem, Item}, mesh::{draw_quad, Mesh}, quad::Quad, structures::{strct::{InserterState, StructureData}, StructureId, Structures}, voxel_world::{chunk::{Chunk, CHUNK_SIZE}, voxel::Voxel}, PhysicsBody};

pub struct VoxelWorld {
    pub chunks: HashMap<IVec3, Chunk>,
    pub structure_blocks: HashMap<IVec3, StructureId>,
    pub dropped_items: Vec<DroppedItem>,
    remesh_queue: Vec<IVec3>,
}


impl VoxelWorld {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            structure_blocks: HashMap::new(),
            dropped_items: vec![],
            remesh_queue: vec![],
        }
    }


    pub fn process(&mut self) {
        let mut n = 0;
        let time = Instant::now();
        while time.elapsed().as_millis() < 3 {
            let Some(chunk_pos) = self.remesh_queue.pop()
            else { break };

            let chunk = self.get_chunk_mut(chunk_pos);

            if chunk.mesh_state == MeshState::Okay {
                continue;
            }

            n += 1;

            const FACE_DIRECTIONS: [(Direction, (i32, i32, i32)); 6] = [
                (Direction::Up,      ( 0,  1,  0)),
                (Direction::Down,    ( 0, -1,  0)),
                (Direction::Right,   (-1,  0,  0)),
                (Direction::Left,    ( 1,  0,  0)),
                (Direction::Forward, ( 0,  0,  1)),
                (Direction::Back,    ( 0,  0, -1)),
            ];

            let mut verticies = vec![];
            let mut indicies = vec![];


            for z in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        let voxel = *chunk.get_usize(x, y, z);

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
                                chunk.get_usize(nx as usize, ny as usize, nz as usize).kind.is_transparent()
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
            chunk.mesh_state = MeshState::Okay;
            chunk.mesh = mesh;
        }

        if n > 0 {
            println!("remeshed {n} chunk in {}ms, {} chunks left", time.elapsed().as_millis_f64(), self.remesh_queue.len());
        }
    }


    pub fn get_chunk(&mut self, pos: IVec3) -> &Chunk {
        self.ensure_chunk_exists(pos);
        self.chunks.get(&pos).unwrap()
    }


    pub fn get_chunk_mut(&mut self, pos: IVec3) -> &mut Chunk {
        self.ensure_chunk_exists(pos);
        let chunk = self.chunks.get_mut(&pos).unwrap();
        chunk.mesh_state = MeshState::ShouldUpdate;
        chunk.is_dirty = true;

        chunk
    }


    pub fn ensure_chunk_exists(&mut self, pos: IVec3) {
        if !self.chunks.contains_key(&pos) {
            let path = format!("saves/chunks/{pos}.chunk");
            println!("hit io");
            match fs::read(&path) {
                Ok(v) => {
                    let mut byte_reader = ByteReader::new(&v).unwrap();
                    let mut chunk = Chunk::empty_chunk();
                    for voxel in &mut chunk.data {
                        let kind = VoxelKind::from_u8(byte_reader.read_u8().unwrap());
                        voxel.kind = kind;
                    }

                    self.chunks.insert(pos, chunk);
                },


                Err(v) => {
                    println!("error while loading chunk file on '{path}': {v}");
                    self.chunks.insert(pos, Chunk::generate(pos));
                }
            };
        }
    }


    pub fn get_mesh(&mut self, pos: IVec3) -> &Mesh {
        let chunk = self.get_chunk(pos);
        if chunk.mesh_state == MeshState::ShouldUpdate {
            let chunk = self.chunks.get_mut(&pos).unwrap();
            chunk.mesh_state = MeshState::Updating;
            self.remesh_queue.push(pos);
        }

        let chunk = self.get_chunk(pos);
        &chunk.mesh
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
                    self.dropped_items.push(DroppedItem::new(item, pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5)));
                }
            }


            match structure.data {
                StructureData::Inserter { state: InserterState::Placing(item), .. } => {
                    self.dropped_items.push(DroppedItem::new(item, pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5)));
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

    pub fn raycast_voxel(&mut self, start: Vec3, dir: Vec3, max_dist: f32) -> Option<(IVec3, IVec3)> {
        let mut pos = start.floor().as_ivec3();
        let step = dir.signum();

        let delta = Vec3::new(
            (1.0 / dir.x).abs(),
            (1.0 / dir.y).abs(),
            (1.0 / dir.z).abs()
        );


        let mut t_max = {
            let fract = start - pos.as_vec3();
            Vec3::new(
                if dir.x > 0.0 { 1.0 - fract.x } else { fract.x } * delta.x,
                if dir.y > 0.0 { 1.0 - fract.y } else { fract.y } * delta.y,
                if dir.z > 0.0 { 1.0 - fract.z } else { fract.z } * delta.z,
            )
        };


        let mut dist = 0.0;
        let mut last_move = Vec3::ZERO;

        while dist < max_dist {
            let voxel = self.get_voxel(pos);

            let is_solid = !voxel.kind.is_air();

            if is_solid {
                return Some((pos, -last_move.normalize().as_ivec3()));
            }

            if t_max.x < t_max.y && t_max.x < t_max.z {
                pos.x += step.x as i32;
                dist = t_max.x;
                t_max.x += delta.x;
                last_move = Vec3::new(step.x, 0.0, 0.0);
            } else if t_max.y < t_max.z {
                pos.y += step.y as i32;
                dist = t_max.y;
                t_max.y += delta.y;
                last_move = Vec3::new(0.0, step.y, 0.0);
            } else {
                pos.z += step.z as i32;
                dist = t_max.z;
                t_max.z += delta.z;
                last_move = Vec3::new(0.0, 0.0, step.z);
            }

        }
        None
    }


    pub fn move_physics_body(&mut self, delta_time: f32, physics_body: &mut PhysicsBody) {
        physics_body.velocity.y -= 9.8 * delta_time;

        let mut position = physics_body.position;


        physics_body.velocity.x *= 1.0 - 2.0 * delta_time;
        physics_body.velocity.z *= 1.0 - 2.0 * delta_time;

        for axis in 0..3 {
            let mut new_position = position;
            new_position[axis] += physics_body.velocity[axis] * delta_time;

            let min = (new_position - physics_body.aabb_dims * 0.5).floor().as_ivec3();
            let max = (new_position + physics_body.aabb_dims * 0.5).ceil().as_ivec3();

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
            if !chunk.is_dirty { continue }
            chunk.is_dirty = false;
            let mut byte_writer = ByteWriter::new();

            for voxel in &chunk.data {
                byte_writer.write_u8(voxel.kind.to_u8());
            }

            let path = format!("saves/chunks/{pos}.chunk");
            fs::write(path, byte_writer.finish()).unwrap();
        }
    }
}


pub fn split_world_pos(pos: IVec3) -> (IVec3, IVec3) {
    let chunk_pos = pos.div_euclid(IVec3::splat(CHUNK_SIZE as i32));
    let chunk_local_pos = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));

    (chunk_pos, chunk_local_pos)
}
