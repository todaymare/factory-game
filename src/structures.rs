pub mod strct;
pub mod work_queue;
pub mod belts;
pub mod inventory;


use glam::{DVec3, IVec3, Mat4, Vec3};
use inventory::StructureInventory;
use sti::{define_key, hash::fxhash::fxhash32};
use strct::{rotate_block_vector, InserterState, Structure, StructureData, StructureKind};
use tracing::warn;
use work_queue::WorkQueue;

use crate::{crafting::{Recipe, FURNACE_RECIPES}, directions::CardinalDirection, gen_map::{KGenMap, KeyGen}, items::{Item, ItemKind}, renderer::Renderer, shader::ShaderProgram, voxel_world::{split_world_pos, voxel::Voxel, VoxelWorld}, Camera, Tick, DROPPED_ITEM_SCALE, TICKS_PER_SECOND};

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

            
            let inventory = structure.inventory.as_mut().unwrap();
            let inventory = &mut inventory.slots;
            match &mut structure.data {
                StructureData::Belt { } => {
                    assert!(output2.is_none());
                    let output = output1;
                    Self::process_lanes(&mut inventory[..4], output);
                },


                StructureData::Splitter { .. } => {
                    for (lane, output) in [output1, output2].into_iter().enumerate() {
                        let inventory = &mut inventory[lane*4..(lane+1)*4];
                        Self::process_lanes(inventory, output);
                    }
                },

                _ => unreachable!(),
            }

        }
    }


    fn process_lanes(inventory: &mut [Option<Item>], mut output: Option<&mut Structure>) {
        for i in 0..4 {
            let lane = i/2;
            let i = i%2;
            let inventory = &mut inventory[lane*2..(lane+1)*2];

            if i > 0 && inventory[i-1].is_none() {
                let item = &mut inventory[i];
                inventory[i-1] = item.take();
                continue;
            }

            let item = &mut inventory[i];
            let Some(output_structure) = &mut output
            else { continue };

            match &mut output_structure.data {
                StructureData::Belt { } => {
                    let inventory = &mut output_structure.inventory.as_mut().unwrap().slots;
                    if inventory[lane * 2 + 1].is_none() {
                        inventory[lane * 2 + 1] = item.take();
                    }
                },


                StructureData::Splitter { priority } => {
                    for side in [0, 1] {
                        let inventory = &mut output_structure.inventory.as_mut().unwrap().slots;
                        let side = (priority[lane] as usize + side) % 2;
                        let inventory = &mut inventory[side*4..(side+1)*4];
                        let inventory = &mut inventory[lane*2..(lane+1)*2];

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
            warn!("tried to update a function that is asleep");
            return;
        }

        let dir = structure.direction;
        let zz = structure.zero_zero();

        match &mut structure.data {
            StructureData::Quarry { current_progress } => {
                let inventory = &mut structure.inventory.as_mut().unwrap();
                debug_assert!(inventory.outputs_len() == 1);

                let (output, _) = inventory.output(0);

                let is_output_empty = output.is_none();
                if !is_output_empty {
                    warn!("can't insert item into inventory. falling back asleep. this is a bug");

                    structure.is_asleep = true;
                    return;
                }


                let x = *current_progress % 3;
                let z = (*current_progress / 3) % 3;
                let y = *current_progress / 9;

                let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
                let pos = rotate_block_vector(dir, pos);

                let voxel = world.get_voxel(zz + pos);

                *current_progress += 1;

                if !voxel.is_air() {
                    let item = world.block_item(structures, zz + pos);

                    world.break_block(structures, zz + pos);

                    let structure = structures.get_mut_without_wake_up(id);
                    let inventory = &mut structure.inventory.as_mut().unwrap();
                    let output = inventory.output_mut(0);

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
                            let Some(mut item) = *input_structure.available_item(index)
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
                            structures.get_mut(*input_structure_id).try_take(index, 1).unwrap();

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
                        if let StructureData::Belt = &mut output_structure.data {
                            let inventory = &mut output_structure.inventory.as_mut().unwrap().slots;
                            let lane = placement_lane(dir, output_structure.direction);
                            let inventory = &mut inventory[lane*2..(lane+1)*2];

                            for index in 0..2 {
                                let slot = &mut inventory[index];
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
                            warn!("inserter's output changed it's mind :(");
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


            StructureData::Assembler { recipe } => {
                let Some(recipe) = recipe
                else { structure.is_asleep = true; return };

                let inventory = structure.inventory.as_mut().unwrap();
                let output = inventory.output_mut(0);
                match output {
                    Some(v) => v.amount += recipe.result.amount,
                    None => *output = Some(recipe.result),
                }

                if try_consume(inventory, *recipe) {
                    let time = recipe.time;
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
                        hardness = (hardness as f32 * quarry_efficiency(pos.y as _)) as u32;
                    }

                    structures.schedule_in(id, hardness);
                    break;
                }
            },


            StructureData::Inserter { .. } => {
                structures.schedule_in(id, 1);
            },


            StructureData::Assembler { recipe } => {
                let Some(recipe) = recipe
                else { structure.is_asleep = true; return };

                let inventory = structure.inventory.as_mut().unwrap();

                if try_consume(inventory, *recipe) {
                    let time = recipe.time;
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



    pub fn render(&self, structures: &Structures, camera: &Camera, renderer: &Renderer, shader: &ShaderProgram) {
        /*
        let kind = self.data.as_kind();

        let position = self.zero_zero();
        let mesh = renderer.meshes.get(kind.item_kind());

        let blocks = self.data.as_kind().blocks(self.direction);
        let mut pos_min = IVec3::MAX;
        let mut pos_max = IVec3::MIN;
        for offset in blocks {
            pos_min = pos_min.min(position + offset);
            pos_max = pos_max.max(position + offset);
        }

        let mesh_position = (pos_min + pos_max).as_dvec3() / 2.0 + DVec3::new(0.5, 0.5, 0.5);
        let mesh_position = (mesh_position - camera.position).as_vec3();

        let mut dims = Vec3::ONE;
        'm: {
        match &self.data {
            StructureData::Belt => {
                dims.y *= 0.7;
                let inventory = &self.inventory.as_ref().unwrap().slots;

                let base = mesh_position + rotate_block_vector(self.direction, IVec3::new(-24, 11, 0)).as_vec3() / 32.0;
                let base = base + Vec3::new(0.0, 0.05, 0.0);

                let mut left_base = base + rotate_block_vector(self.direction, IVec3::new(0, 0, -1)).as_vec3() * 0.3;
                for item in &inventory[..2] {
                    left_base += rotate_block_vector(self.direction, IVec3::new(1, 0, 0)).as_vec3() * 0.5;
                    if let Some(item) = item {
                        let rot = if matches!(item.kind, ItemKind::Structure(_)) { 0.0 }
                                  else { 90f32.to_radians() };
                        renderer.draw_item(shader, item.kind, left_base, Vec3::splat(DROPPED_ITEM_SCALE), Vec3::new(rot, 0.0, 0.0));
                    }
                }

                let mut right_base = base + rotate_block_vector(self.direction, IVec3::new(0, 0, 1)).as_vec3() * 0.3;
                for item in &inventory[2..4] {
                    right_base += rotate_block_vector(self.direction, IVec3::new(1, 0, 0)).as_vec3() * 0.5;
                    if let Some(item) = item {
                        let rot = if matches!(item.kind, ItemKind::Structure(_)) { 0.0 }
                                  else { 90f32.to_radians() };
                        renderer.draw_item(shader, item.kind, right_base, Vec3::splat(DROPPED_ITEM_SCALE), Vec3::new(rot, 0.0, 0.0));
                    }
                }
            }


           StructureData::Splitter { .. } => {
                dims.y *= 0.7;
                let inventory = &self.inventory.as_ref().unwrap().slots;

                let base = mesh_position + rotate_block_vector(self.direction, IVec3::new(-24, 11, 16)).as_vec3() / 32.0;
                let base = base + Vec3::new(0.0, 0.05, 0.0);

                let mut left_base = base + rotate_block_vector(self.direction, IVec3::new(0, 0, -1)).as_vec3() * 0.3;
                for item in &inventory[..2] {
                    left_base += rotate_block_vector(self.direction, IVec3::new(1, 0, 0)).as_vec3() * 0.5;
                    if let Some(item) = item {
                        let rot = if matches!(item.kind, ItemKind::Structure(_)) { 0.0 }
                                  else { 90f32.to_radians() };
                        renderer.draw_item(shader, item.kind, left_base, Vec3::splat(DROPPED_ITEM_SCALE), Vec3::new(rot, 0.0, 0.0));
                    }
                }

                let mut right_base = base + rotate_block_vector(self.direction, IVec3::new(0, 0, 1)).as_vec3() * 0.3;
                for item in &inventory[2..4] {
                    right_base += rotate_block_vector(self.direction, IVec3::new(1, 0, 0)).as_vec3() * 0.5;
                    if let Some(item) = item {
                        let rot = if matches!(item.kind, ItemKind::Structure(_)) { 0.0 }
                                  else { 90f32.to_radians() };
                        renderer.draw_item(shader, item.kind, right_base, Vec3::splat(DROPPED_ITEM_SCALE), Vec3::new(rot, 0.0, 0.0));
                    }
                }

                let base = mesh_position + rotate_block_vector(self.direction, IVec3::new(-24, 11, -16)).as_vec3() / 32.0;
                let base = base + Vec3::new(0.0, 0.05, 0.0);

                let mut left_base = base + rotate_block_vector(self.direction, IVec3::new(0, 0, -1)).as_vec3() * 0.3;
                for item in &inventory[4..6] {
                    left_base += rotate_block_vector(self.direction, IVec3::new(1, 0, 0)).as_vec3() * 0.5;
                    if let Some(item) = item {
                        let rot = if matches!(item.kind, ItemKind::Structure(_)) { 0.0 }
                                  else { 90f32.to_radians() };
                        renderer.draw_item(shader, item.kind, left_base, Vec3::splat(DROPPED_ITEM_SCALE), Vec3::new(rot, 0.0, 0.0));
                    }
                }

                let mut right_base = base + rotate_block_vector(self.direction, IVec3::new(0, 0, 1)).as_vec3() * 0.3;
                for item in &inventory[6..8] {
                    right_base += rotate_block_vector(self.direction, IVec3::new(1, 0, 0)).as_vec3() * 0.5;
                    if let Some(item) = item {
                        let rot = if matches!(item.kind, ItemKind::Structure(_)) { 0.0 }
                                  else { 90f32.to_radians() };
                        renderer.draw_item(shader, item.kind, right_base, Vec3::splat(DROPPED_ITEM_SCALE), Vec3::new(rot, 0.0, 0.0));
                    }
                }
            }


            StructureData::Assembler { recipe }=> {
                let Some(recipe) = recipe
                else { break 'm };

                let hash = fxhash32(&self.position) % 1024;
                let t = (hash + structures.current_tick.u32()) as f32 / TICKS_PER_SECOND as f32;
                let rotation = Vec3::new(t, t * 0.7, t * 1.3);

                renderer.draw_item(shader, recipe.result.kind, mesh_position, Vec3::splat(1.2), rotation);
            }
            _ => (),
        }
        }

        let rot = self.direction.as_ivec3().as_vec3();
        let rot = rot.x.atan2(rot.z);
        let rot = rot + 90f32.to_radians();
        let model = Mat4::from_translation(mesh_position) * Mat4::from_scale(dims) * Mat4::from_rotation_y(rot);
        shader.set_matrix4(c"model", model);
        mesh.draw();
        */
    }
}


pub fn try_consume(inventory: &mut StructureInventory, recipe: Recipe) -> bool {
    let (output_slot, output_meta) = inventory.output(0);
    if let Some(output) = output_slot
        && output.amount + recipe.result.amount > output_meta.max_amount {
        return false;
    }


    let input_len = recipe.requirements.len();
    for index in 0..input_len {
        let Some(inv_item) = inventory.slots[index]
        else { return false };

        let recipe_item = recipe.requirements[index];

        if inv_item.amount < recipe_item.amount {
            return false;
        }
    }


    let input_len = recipe.requirements.len();
    for index in 0..input_len {
        let Some(inv_item) = &mut inventory.slots[index]
        else { unreachable!() };

        let recipe_item = recipe.requirements[index];
        inv_item.amount -= recipe_item.amount;
        if inv_item.amount == 0 {
            inventory.slots[index] = None;
        }
    }

    true
}


pub fn quarry_efficiency(y_pos: f32) -> f32 {
    if y_pos > 0.0 { return 1.0 }
    1.0 + (y_pos * 0.005).powi(2)
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
        (CD::North, CD::East) => 0,
        (CD::North, CD::West) => 1,
        (CD::South, CD::East) => 1,
        (CD::South, CD::West) => 0,
        (CD::East, CD::North) => 1,
        (CD::East, CD::South) => 0,
        (CD::West, CD::North) => 0,
        (CD::West, CD::South) => 1,
        _ => 0,
    }
}
