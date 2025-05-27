pub mod chunk;
pub mod voxel;

use std::{collections::{HashMap, VecDeque}, ops::Bound};

use glam::{IVec3, Vec3};
use voxel::VoxelKind;

use crate::{items::{DroppedItem, Item}, structures::{strct::Structure, StructureId, Structures}, voxel_world::{chunk::{Chunk, CHUNK_SIZE}, voxel::Voxel}, PhysicsBody};

pub struct VoxelWorld {
    pub chunks: HashMap<IVec3, Chunk>,
    pub structure_blocks: HashMap<IVec3, StructureId>,
    pub dropped_items: Vec<DroppedItem>,
}


impl VoxelWorld {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            structure_blocks: HashMap::new(),
            dropped_items: vec![],
        }
    }


    pub fn get_chunk(&mut self, pos: IVec3) -> &mut Chunk {
        if !self.chunks.contains_key(&pos) {
            self.chunks.insert(pos, Chunk::generate(pos));
        }

        self.chunks.get_mut(&pos).unwrap()
    }


    pub fn get_voxel(&mut self, pos: IVec3) -> &Voxel {
        let (chunk_pos, chunk_local_pos) = split_world_pos(pos);

        self.get_chunk(chunk_pos).get(chunk_local_pos)
    }


    pub fn get_voxel_mut(&mut self, pos: IVec3) -> &mut Voxel {
        let (chunk_pos, chunk_local_pos) = split_world_pos(pos);
        self.get_chunk(chunk_pos).get_mut(chunk_local_pos)
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
            let structure = structures.remove(structure_id);
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


            /*
            if let Some(inputs) = structure.input {
                for slot in inputs.iter() {
                    let Some(item) = slot.item
                    else { continue };

                    self.dropped_items.push(DroppedItem::new(item, pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5)));

                }
            }


            if let Some(output) = structure.output {
                if let Some(item) = output.item {
                    self.dropped_items.push(DroppedItem::new(item, pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5)));
                }
            }
            */

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

}


pub fn split_world_pos(pos: IVec3) -> (IVec3, IVec3) {
    let chunk_pos = pos.div_euclid(IVec3::splat(CHUNK_SIZE as i32));
    let chunk_local_pos = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));

    (chunk_pos, chunk_local_pos)
}
