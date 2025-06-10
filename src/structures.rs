pub mod strct;
pub mod work_queue;
pub mod belts;

use std::hash::{DefaultHasher, Hash, Hasher};

use belts::{Belts, Node};
use glam::{DVec3, IVec3, Mat4, Vec3};
use rand::seq::IndexedRandom;
use sti::define_key;
use strct::{rotate_block_vector, InserterState, Structure, StructureData, StructureKind};
use work_queue::WorkQueue;

use crate::{crafting::{Recipe, FURNACE_RECIPES}, directions::CardinalDirection, gen_map::{KGenMap, KeyGen}, items::Item, renderer::Renderer, shader::ShaderProgram, voxel_world::{split_world_pos, voxel::Voxel, VoxelWorld}, Camera, Tick, DROPPED_ITEM_SCALE};

define_key!(pub StructureKey(u32));
define_key!(pub StructureGen(u32));


#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash)]
pub struct StructureId(pub KeyGen<StructureGen, StructureKey>);



pub struct Structures {
    pub structs: KGenMap<StructureGen, StructureKey, Structure>,
    pub work_queue: WorkQueue,
    pub to_be_awoken: Vec<StructureId>,
    pub current_tick: Tick,
}


#[derive(PartialEq, Eq, Hash, Debug)]
pub struct Crafter {
    pub recipe: Recipe,
    pub inventory: Vec<Item>,
    pub output: Item,
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


    pub fn get_mut_without_wake_up(&mut self, id: StructureId) -> &mut Structure {
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
        if self.current_tick.0 % 5 == 0 {
            self.update_belts(world);
        }

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


    pub fn add_structure(&mut self, world: &mut VoxelWorld, structure: Structure) -> StructureId {
        let id = self.insert(structure);
        let structure = &mut self.structs[id.0];

        let placement_origin = structure.zero_zero();

        let blocks = structure.data.as_kind().blocks(structure.direction);
        for offset in blocks {
            let pos = placement_origin + offset;
            let (chunk_pos, voxel_pos) = split_world_pos(pos);
            let chunk = world.get_chunk_mut(chunk_pos);
            *chunk.get_mut(voxel_pos) = Voxel::StructureBlock;
            chunk.persistent = true;
            world.structure_blocks.insert(pos, id);
        }

        self.to_be_awoken.push(id);
        id
    }


    fn update_belts(&mut self, world: &mut VoxelWorld) {
        let belts = self.belts(world);


        // we iterate in reverse because belts
        // update from the last node to the first
        for &node in belts.worklist.iter().rev() {
            let node = belts.node(node);

            // extract out the references
            let [structure, output1, output2] = match node.outputs {
                [None, None] => {
                    let structure = self.structs.get_mut(node.structure_id.0);
                    [structure, None, None]
                },

                [Some(o1), None] => {
                    let o1 = belts.node(o1);
                    let [s, o1] = self.structs.get_many_mut(
                        [node.structure_id.0, o1.structure_id.0]);

                    [s, o1, None]
                },

                [Some(o1), Some(o2)] => {
                    let o1 = belts.node(o1);
                    let o2 = belts.node(o2);
                    self.structs.get_many_mut([
                        node.structure_id.0,
                        o1.structure_id.0,
                        o2.structure_id.0
                    ])
                }

                _ => unreachable!(),
            };


            let Some(structure) = structure
            else { unreachable!() };

            
            match &mut structure.data {
                StructureData::Belt { inventory } => {
                    assert!(output2.is_none());
                    let output = output1;
                    Self::process_lanes(inventory, output);
                },


                StructureData::Splitter { inventory, .. } => {
                    for (i, output) in [output1, output2].into_iter().enumerate() {
                        let inventory = &mut inventory[i];
                        Self::process_lanes(inventory, output);
                    }
                },

                _ => unreachable!(),
            }

        }
    }


    fn process_lanes(inventory: &mut [[Option<Item>; 2]; 2], mut output: Option<&mut Structure>) {
        for (lane, inventory) in inventory.iter_mut().enumerate() {
            for i in 0..inventory.len() {
                if i > 0 && inventory[i-1].is_none() {
                    let item = &mut inventory[i];
                    inventory[i-1] = item.take();
                    continue;
                }

                let item = &mut inventory[i];
                let Some(output_structure) = &mut output
                else { continue };

                match &mut output_structure.data {
                    StructureData::Belt { inventory } => {
                        if inventory[lane][1].is_none() {
                            inventory[lane][1] = item.take();
                        }
                    },


                    StructureData::Splitter { inventory, priority } => {
                        for side in [0, 1] {
                            let inventory = &mut inventory[(priority[lane] as usize + side) % 2];
                            let inventory = &mut inventory[lane];

                            let slot = &mut inventory[1];
                            if slot.is_none() {
                                *slot = item.take();
                                priority[lane] += 1;
                                priority[lane] %= 2;
                            }

                        }
                    },

                    _ => unreachable!(),
                }
            }
        }
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

impl Structure {
    pub fn update(id: StructureId, structures: &mut Structures, world: &mut VoxelWorld) {
        let structure = structures.get_mut_without_wake_up(id);
        if structure.is_asleep {
            println!("[warn] tried to update a function that is asleep");
            return;
        }
        let dir = structure.direction;
        let zz = structure.zero_zero();

        match &mut structure.data {
            StructureData::Quarry { current_progress, output } => {
                let x = *current_progress % 3;
                let z = (*current_progress / 3) % 3;
                let y = *current_progress / 9;

                let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
                let pos = rotate_block_vector(dir, pos);

                let voxel = world.get_voxel(zz + pos);

                let is_output_empty = output.is_none();
                if !is_output_empty {
                    println!("[warn] can't insert item into inventory. falling back asleep. this is a bug");

                    structure.is_asleep = true;
                    return;
                }

                *current_progress += 1;

                if !voxel.is_air() {
                    let item = world.block_item(structures, zz + pos);

                    world.break_block(structures, zz + pos);

                    let structure = structures.get_mut_without_wake_up(id);
                    let StructureData::Quarry { output, .. } = &mut structure.data
                    else { unreachable!() };

                    *output = Some(item);
                    structure.is_asleep = true;
                }
            },


            StructureData::Inserter { state, filter } => {
                let mut final_state = InserterState::Searching;

                let output_structure_position = zz + rotate_block_vector(structure.direction, IVec3::new(-1, 0, 0));
                let input_structure_position = zz + rotate_block_vector(structure.direction, IVec3::new(3, 0, 0));
                let filter = *filter;


                'body: { match state {
                    InserterState::Searching => {
                        let Some(input_structure_id) = world.structure_blocks.get(&input_structure_position)
                        else { break 'body };
                        let Some(output_structure_id) = world.structure_blocks.get(&output_structure_position)
                        else { break 'body };

                        let input_structure = structures.get(*input_structure_id);


                        let available_items_len = input_structure.available_items_len();
                        for index in 0..available_items_len {
                            let input_structure = structures.get(*input_structure_id);
                            let Some(mut item) = input_structure.available_item(index)
                            else {
                                // no item in this index
                                continue;
                            };

                            if let Some(filter) = filter {
                                if filter != item.kind {
                                    continue;
                                }
                            }

                            item.amount = 1;

                            let output_structure = structures.get(*output_structure_id);
                            if !output_structure.can_accept(item)
                                && output_structure.data.as_kind() != StructureKind::Belt {
                                // better luck next time
                                continue;
                            }

                            // yippie!
                            structures.get_mut(*input_structure_id).try_take(index).unwrap();

                            final_state = InserterState::Placing(item);
                            break 'body;
                        }

                        final_state = InserterState::Searching;
                    }

                    InserterState::Placing(item) => {
                        let Some(output_structure_id) = world.structure_blocks.get(&output_structure_position)
                        else { break 'body };

                        let item = *item;
                        let output_structure = structures.get_mut(*output_structure_id);
                        if let StructureData::Belt { inventory } = &mut output_structure.data {
                            let lane = placement_lane(dir, output_structure.direction);
                            let inv = &mut inventory[lane];

                            for slot in inv {
                                if slot.is_none() {
                                    *slot = Some(item);
                                    final_state = InserterState::Searching;
                                    break 'body;
                                }
                            }

                            structures.schedule_in(id, 10);
                            return;
                        }


                        if !output_structure.can_accept(item) {
                            println!("[warn] inserter's output changed it's mind :(");
                            structures.schedule_in(id, 10);
                            return;
                        }

                        output_structure.give_item(item);

                        let structure = structures.get_mut_without_wake_up(id);

                        let StructureData::Inserter { state, .. } = &mut structure.data
                        else { unreachable!() };

                        *state = InserterState::Searching;
                        Structure::update(id, structures, world);
                        return;
                    },

                } }

                let structure = structures.get_mut_without_wake_up(id);

                let StructureData::Inserter { state, .. } = &mut structure.data
                else { unreachable!() };

                *state = final_state;

                match state {
                    strct::InserterState::Searching => structures.schedule_in(id, 10),
                    strct::InserterState::Placing(_) => structures.schedule_in(id, 20),
                };

            },


            StructureData::Assembler { crafter } => {
                crafter.output.amount += 1;
                if crafter.try_consume() {
                    let time = crafter.recipe.time;
                    structures.schedule_in(id, time);
                } else {
                    structure.is_asleep = true;
                }
            }


            StructureData::Furnace { input, output } => {
                if let Some(input_item) = input {
                    let Some(recipe) = FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == input_item.kind)
                    else { unreachable!() };

                    if let Some(output) = output {
                        assert_eq!(recipe.result.kind, output.kind);
                        output.amount += recipe.result.amount;

                    } else {
                        *output = Some(recipe.result);
                    }
                }

                if let Some(input_item) = input {
                    let Some(recipe) = FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == input_item.kind)
                    else { unreachable!() };

                    if let Some(output) = output {
                        if output.kind != recipe.result.kind 
                            || output.amount + recipe.result.amount > output.kind.max_stack_size() {
                            structure.is_asleep = true;
                            return;
                        }
                    }

                    input_item.amount -= 1;
                    if input_item.amount == 0 {
                        *input = None;
                    }

                    structures.schedule_in(id, recipe.time);
                    return;
                }  

                structure.is_asleep = true;
            }


            StructureData::Chest { .. } => {},
            StructureData::Silo { .. } => {},
            StructureData::Belt { .. } => {},
            StructureData::Splitter { .. } => {},
        }
    }


    pub fn wake_up(id: StructureId, structures: &mut Structures, world: &mut VoxelWorld) {
        let structure = structures.get_mut_without_wake_up(id);
        assert!(structure.is_asleep);

        let dir = structure.direction;
        let zz = structure.zero_zero();

        let structure = structures.get_mut_without_wake_up(id);
        structure.is_asleep = false;

        match &mut structure.data {
            StructureData::Quarry { current_progress, .. } => {
                loop {

                    let x = *current_progress % 3;
                    let z = (*current_progress / 3) % 3;
                    let y = *current_progress / 9;
                    let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
                    let pos = rotate_block_vector(dir, pos);
                    let voxel = world.get_voxel(zz + pos);

                    if voxel.is_air() {
                        *current_progress += 1;
                        continue;
                    }

                    let mut hardness = voxel.base_hardness();
                    if pos.y < 0 { 
                        hardness = (hardness as f32 * (1.0 + (pos.y as f32 * 0.01).powi(2))) as u32;
                    }

                    structures.schedule_in(id, hardness);
                    break;
                }

            },


            StructureData::Inserter { .. } => {
                structures.schedule_in(id, 1);
            },


            StructureData::Assembler { crafter } => {
                if crafter.try_consume() {
                    let time = crafter.recipe.time;
                    structures.schedule_in(id, time);
                } else {
                    structure.is_asleep = true;
                }
            }


            StructureData::Furnace { input, output } => {
                if let Some(input_item) = input {
                    let Some(recipe) = FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == input_item.kind)
                    else { unreachable!() };

                    if let Some(output) = output {
                        if output.kind != recipe.result.kind 
                            || output.amount + recipe.result.amount > output.kind.max_stack_size() {
                            structure.is_asleep = true;
                            return;
                        }
                    }

                    input_item.amount -= 1;
                    if input_item.amount == 0 {
                        *input = None;
                    }


                    structures.schedule_in(id, recipe.time);
                    return;
                }

                structure.is_asleep = true;
            }


            StructureData::Chest { .. } => {}
            StructureData::Silo { .. } => {}
            StructureData::Belt { .. } => {}
            StructureData::Splitter { .. } => {}
        }
    }



    pub fn render(&self, _: &Structures, camera: &Camera, renderer: &Renderer, shader: &ShaderProgram) {
        let kind = self.data.as_kind();

        let position = self.position - self.data.as_kind().origin(self.direction);
        let mesh = renderer.meshes.get(kind.item_kind());

        let blocks = self.data.as_kind().blocks(self.direction);
        let mut min = IVec3::MAX;
        let mut max = IVec3::MIN;
        for offset in blocks {
            min = min.min(position + offset);
            max = max.max(position + offset);
        }

        let mesh_position = (min + max).as_dvec3() / 2.0 + DVec3::new(0.5, 0.0, 0.5);
        let mesh_position = (mesh_position - camera.position).as_vec3();

        let rot = self.direction.as_ivec3().as_vec3();
        let rot = rot.x.atan2(rot.z);
        let rot = rot + 90f32.to_radians();
        let model = Mat4::from_translation(mesh_position) * Mat4::from_rotation_y(rot);
        shader.set_matrix4(c"model", model);
        mesh.draw();

        match &self.data {
            StructureData::Chest { inventory } => {
                for (i, slot) in inventory.iter().enumerate() {
                    let Some(item) = slot
                    else { continue };

                    let mut hash = DefaultHasher::new();
                    self.position.hash(&mut hash);
                    i.hash(&mut hash);
                    item.kind.hash(&mut hash);
                    let pos = hash.finish() % 1000;
                    let x = (pos % 81) as f32 / 81.0;
                    let y = (pos % 96) as f32 / 96.0;
                    let z = (pos % 27) as f32 / 27.0;
                    let pos = mesh_position + Vec3::new(x, y, z) * 0.9 + Vec3::new(-0.45, 0.0, -0.45);

                    renderer.draw_item(shader, item.kind, pos, Vec3::splat(0.1), 0.00);
                }
            },


            StructureData::Belt { inventory, .. } => {
                let base = mesh_position + rotate_block_vector(self.direction, IVec3::new(-3, 3, 0)).as_vec3() / 4.0;
                let mut left_base = base + rotate_block_vector(self.direction, IVec3::new(0, 0, -1)).as_vec3() * 0.3;
                for item in inventory[1] {
                    left_base += rotate_block_vector(self.direction, IVec3::new(1, 0, 0)).as_vec3() * 0.5;
                    if let Some(item) = item {
                        renderer.draw_item(shader, item.kind, left_base, Vec3::splat(DROPPED_ITEM_SCALE), 0.0);
                    }
                }

                let mut right_base = base + rotate_block_vector(self.direction, IVec3::new(0, 0, 1)).as_vec3() * 0.3;
                for item in inventory[0] {
                    right_base += rotate_block_vector(self.direction, IVec3::new(1, 0, 0)).as_vec3() * 0.5;
                    if let Some(item) = item {
                        renderer.draw_item(shader, item.kind, right_base, Vec3::splat(DROPPED_ITEM_SCALE), 0.0);
                    }
                }
            }
            _ => (),
        }
    }
}


impl Crafter {
    pub fn from_recipe(recipe: Recipe) -> Self {
        let mut items = Vec::with_capacity(recipe.requirements.len());
        for item in recipe.requirements {
            let mut item = *item;
            item.amount = 0;

            items.push(item);
        }

        let mut output = recipe.result;
        output.amount = 0;

        Self {
            recipe,
            inventory: items,
            output,
        }
    }


    pub fn try_consume(&mut self) -> bool {
        if self.output.amount >= self.recipe.result.amount * 2 { return false }
        for i in 0..self.inventory.len() {
            let recipe_item = self.recipe.requirements[i];
            let inv_item = self.inventory[i];
            if inv_item.amount < recipe_item.amount {
                return false;
            }
        }
    

        for i in 0..self.inventory.len() {
            let recipe_item = self.recipe.requirements[i];
            let inv_item = &mut self.inventory[i];
            inv_item.amount -= recipe_item.amount;
        }
        true
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



fn placement_lane(inserter_dir: CardinalDirection, belt_dir: CardinalDirection) -> usize {
    use CardinalDirection as CD;

    match (inserter_dir, belt_dir) {
        (CD::North, CD::East) => 1,
        (CD::North, CD::West) => 0,
        (CD::South, CD::East) => 0,
        (CD::South, CD::West) => 1,
        (CD::East, CD::North) => 0,
        (CD::East, CD::South) => 1,
        (CD::West, CD::North) => 1,
        (CD::West, CD::South) => 0,
        _ => 0,
    }
}
