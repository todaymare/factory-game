pub mod strct;
pub mod work_queue;

use std::{collections::BTreeMap, ops::Bound};

use glam::{IVec3, Mat4, Vec3};
use sti::{define_key, println, vec::KVec};
use strct::{rotate_block_vector, Structure, StructureData};
use work_queue::WorkQueue;

use crate::{directions::CardinalDirection, gen_map::{KGenMap, KeyGen}, items::{Item, ItemKind, ItemMeshes}, mesh::Mesh, renderer::Renderer, shader::ShaderProgram, voxel_world::{voxel::VoxelKind, VoxelWorld}, Game, Tick};

define_key!(pub StructureKey(u32));
define_key!(pub StructureGen(u32));


#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub struct StructureId(pub KeyGen<StructureGen, StructureKey>);



pub struct Structures {
    structs: KGenMap<StructureGen, StructureKey, Structure>,
    pub work_queue: WorkQueue,
    to_be_awoken: Vec<StructureId>,
    current_tick: Tick,
}


impl Structures {
    pub fn new() -> Self {
        Self {
            structs: KGenMap::new(),
            work_queue: WorkQueue::new(),
            current_tick: Tick::initial(),
            to_be_awoken: vec![], 
        }
    }


    pub fn insert(&mut self, structure: Structure) -> StructureId {
        StructureId(self.structs.insert(structure))
    }


    pub fn remove(&mut self, id: StructureId) -> Structure {
        self.structs.remove(id.0)
    }


    pub fn get(&self, id: StructureId) -> &Structure {
        &self.structs[id.0]
    }


    pub fn for_each<F: Fn(&Structure)>(&self, f: F) {
        self.structs.for_each(f);
    }


    pub fn get_mut(&mut self, id: StructureId) -> &mut Structure {
        let strct = &mut self.structs[id.0];

        if strct.is_asleep {
            self.to_be_awoken.push(id);
        }

        let strct = &mut self.structs[id.0];
        strct
    }


    pub fn get_mut_no_schedule(&mut self, id: StructureId) -> &mut Structure {
        let strct = &mut self.structs[id.0];
        strct
    }


    pub fn schedule_in(&mut self, id: StructureId, ticks: u32) -> Tick {
        let tick = self.current_tick + Tick::new(ticks); 
        self.work_queue.entries.insert((tick, id), ());
        tick
    }


    pub fn process(&mut self, world: &mut VoxelWorld) {
        self.current_tick = self.current_tick.inc();
        let to_be_updated = self.work_queue.process(self.current_tick);

        let mut to_be_awoken = core::mem::take(&mut self.to_be_awoken);
        to_be_awoken.sort();
        to_be_awoken.dedup();
        for id in to_be_awoken {
            Structure::wake_up(id, self, world);
        }

        for id in to_be_updated {
            Structure::update(id.1, self, world);
        }
    }


    pub fn add_structure(&mut self, world: &mut VoxelWorld, structure: Structure) {
        let id = self.insert(structure);
        let structure = self.get(id);

        let placement_origin = structure.zero_zero();

        let blocks = structure.data.as_kind().blocks(structure.direction);
        for offset in blocks {
            let pos = placement_origin + offset;
            world.get_voxel_mut(pos).kind = VoxelKind::StructureBlock;
            world.structure_blocks.insert(pos, id);
        }

        self.to_be_awoken.push(id);
    }

}


impl PartialOrd for StructureId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.key.partial_cmp(&other.0.key)
    }
}


impl Ord for StructureId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.key.cmp(&other.0.key)
    }
}


#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub item: Option<Item>,
    pub expected: Option<ItemKind>,
    pub max: u32,
}


impl Slot {
    pub fn can_give(&self, item: Item) -> bool {
        if let Some(current_item) = self.item {
            if current_item.kind != item.kind { return false }
            if current_item.amount + item.amount > self.max {
                return false
            }

            return true;
        } else if let Some(expected) = self.expected {
            if expected != item.kind { return false }
            if item.amount > self.max { return false }
            return true;
        } else {
            return item.amount <= self.max;
        }
    }


    pub fn give(&mut self, item: Item) {
        assert!(self.can_give(item));
        if let Some(item) = &mut self.item {
            item.amount += item.amount;
        } else {
            self.item = Some(item);
        }
    }
}


impl Structure {
    pub fn update(id: StructureId, structures: &mut Structures, world: &mut VoxelWorld) {
        let structure = structures.get_mut_no_schedule(id);
        if structure.is_asleep {
            println!("[warn] tried to update a function that is asleep");
            return;
        }
        let dir = structure.direction;
        let zz = structure.zero_zero();
        let output = structure.output;

        match &mut structure.data {
            StructureData::Quarry { current_progress } => {
                let x = *current_progress % 3;
                let z = (*current_progress / 3) % 3;
                let y = *current_progress / 9;
                
                let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
                let pos = rotate_block_vector(dir, pos);

                let item = world.block_item(structures, zz + pos);
                let voxel = world.get_voxel(zz + pos);

                if !voxel.kind.is_air() {
                    if output.unwrap().can_give(item) {
                        world.break_block(structures, zz + pos);
                        let structure = structures.get_mut_no_schedule(id);
                        structure.output.as_mut().unwrap().give(item);
                    } else {
                        println!("[warn] can't insert item into inventory. falling back asleep. this is a bug");
                        let structure = structures.get_mut_no_schedule(id);
                        structure.is_asleep = true;
                        return;
                    }
                }

                // prepare the next mining phase
                let structure = structures.get_mut_no_schedule(id);
                let StructureData::Quarry { current_progress } = &mut structure.data
                else { unreachable!() };

                let output = structure.output.unwrap();

                loop {
                    *current_progress += 1;

                    let x = *current_progress % 3;
                    let z = (*current_progress / 3) % 3;
                    let y = *current_progress / 9;
                    let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
                    let pos = rotate_block_vector(dir, pos);
                    let voxel = world.get_voxel(zz + pos);

                    if voxel.kind.is_air() { continue };

                    let mut hardness = voxel.kind.base_hardness();
                    if pos.y < 0 { 
                        hardness = (hardness as f32 * (1.0 + (pos.y as f32 * 0.01).powi(2))) as u32;
                    }

                    if output.can_give(item) {
                        structures.schedule_in(id, hardness);
                    } else {
                        let structure = structures.get_mut_no_schedule(id);
                        structure.is_asleep = true;
                        return;
                    }

                    break;
                }

            },


            StructureData::Inserter => (),
        }
    }


    pub fn wake_up(id: StructureId, structures: &mut Structures, world: &mut VoxelWorld) {
        let structure = structures.get_mut_no_schedule(id);
        assert!(structure.is_asleep);

        let dir = structure.direction;
        let zz = structure.zero_zero();

        let structure = structures.get_mut_no_schedule(id);
        structure.is_asleep = false;

        let StructureData::Quarry { current_progress } = &mut structure.data
        else { return  };

        loop {

            let x = *current_progress % 3;
            let z = (*current_progress / 3) % 3;
            let y = *current_progress / 9;
            let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
            let pos = rotate_block_vector(dir, pos);
            let voxel = world.get_voxel(zz + pos);

            if voxel.kind.is_air() {
                *current_progress += 1;
                continue;
            }

            let mut hardness = voxel.kind.base_hardness();
            if pos.y < 0 { 
                hardness = (hardness as f32 * (1.0 + (pos.y as f32 * 0.01).powi(2))) as u32;
            }

            structures.schedule_in(id, hardness);
            break;
        }

        


    }



    pub fn render(&self, _: &Renderer, meshes: &ItemMeshes, shader: &ShaderProgram) {
        let kind = self.data.as_kind();

        let position = self.position - self.data.as_kind().origin(self.direction);
        let mesh = meshes.get(kind.item_kind());

        let blocks = self.data.as_kind().blocks(self.direction);
        let mut min = IVec3::MAX;
        let mut max = IVec3::MIN;
        for offset in blocks {
            min = min.min(position + offset);
            max = max.max(position + offset);
        }

        let mesh_position = (min + max).as_vec3() / 2.0 + Vec3::new(0.5, 0.0, 0.5);

        let rot = self.direction.as_ivec3().as_vec3();
        let rot = rot.x.atan2(rot.z);
        let rot = rot + 90f32.to_radians();
        let model = Mat4::from_translation(mesh_position) * Mat4::from_rotation_y(rot);
        shader.set_matrix4(c"model", model);

        mesh.draw();
    }


}


pub fn rotate_vector(direction: Vec3, v: Vec3) -> IVec3 {
    let angle = direction.x.atan2(direction.z);
    let cos = angle.cos();
    let sin = angle.sin();

    let x = v.x as f32;
    let y = v.y as f32;
    let z = v.z as f32;

    let rotated_x = cos * x - sin * z;
    let rotated_z = sin * x + cos * z;

    IVec3::new(
        rotated_x.round() as i32,
        y.round() as i32,
        rotated_z.round() as i32,
    )
}



#[test]
fn test_work_queue() {
    let mut wq = WorkQueue { entries: BTreeMap::new() };

    let k1 = StructureId(KeyGen::new(StructureGen(0), StructureKey(1)));
    let k2 = StructureId(KeyGen::new(StructureGen(0), StructureKey(2)));
    let k3 = StructureId(KeyGen::new(StructureGen(0), StructureKey(3)));
    let k4 = StructureId(KeyGen::new(StructureGen(0), StructureKey(4)));

    /*
    wq.insert(10, k1);
    wq.insert(15, k2);
    wq.insert(20, k4);
    wq.insert(20, k3);
    */

    assert_eq!(&*wq.process(Tick::new(9)), &[]);
    assert_eq!(&*wq.process(Tick::new(10)), &[(Tick::new(10), k1)]);
    assert_eq!(&*wq.process(Tick::new(17)), &[(Tick::new(15), k2)]);
    assert_eq!(&*wq.process(Tick::new(25)), &[(Tick::new(20), k3), (Tick::new(20), k4)]);
}



