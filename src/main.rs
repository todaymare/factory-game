#![feature(duration_millis_float)]
#![feature(portable_simd)]
#![feature(btree_cursors)]

pub mod shader;
pub mod mesh;
pub mod chunk;
pub mod quad;
pub mod renderer;
pub mod input;
pub mod items;
pub mod structure;
pub mod gen_map;

use core::{f32, time};
use std::{char, collections::{HashMap, HashSet}, f32::consts::{PI, TAU}, fmt::{Display, Write}, fs, io::BufReader, ops::Bound, simd::f32x4, time::Instant};

use chunk::{Chunk, Voxel, VoxelKind, CHUNK_SIZE};
use gen_map::KGenMap;
use gl::BindFragDataLocationIndexed::load_with;
use glam::{IVec2, IVec3, Mat4, Vec2, Vec3, Vec4};
use glfw::{GlfwReceiver, Key, MouseButton, WindowEvent};
use input::InputManager;
use items::{DroppedItem, Item, ItemKind, ItemMeshes};
use mesh::{Mesh, Vertex};
use obj::Obj;
use rand::{random, seq::IndexedRandom};
use renderer::Renderer;
use shader::{Shader, ShaderProgram};
use sti::{define_key, println, vec::KVec};
use structure::{CompassDirection, Structure, StructureData, StructureGen, StructureId, StructureKey, StructureKind, WorkQueue};

define_key!(EntityId(u32));


const MOUSE_SENSITIVITY : f32 = 0.1;

const PLAYER_REACH : f32 = 5.0;
const PLAYER_SPEED : f32 = 5.0;
const PLAYER_INTERACT_DELAY : f32 = 0.2;

const RENDER_DISTANCE : i32 = 1;

const DROPPED_ITEM_SCALE : f32 = 0.25;


fn main() {
    let mut game = Game {
        world: World {
            chunks: HashMap::new(),
            camera: Camera {
                position: Vec3::ZERO,
                direction: Vec3::Z,
                up: Vec3::new(0.0, 1.0, 0.0),
                pitch: 0.0,
                yaw: 90.0f32.to_radians(),

            },
        },

        work_queue: WorkQueue::new(),
        structures: KGenMap::new(),
        structure_blocks: HashMap::new(),

        player: Player {
            body: PhysicsBody {
                position: Vec3::new(0.0, 10.0, 0.0),
                velocity: Vec3::ZERO,
                aabb_dims: Vec3::new(0.8, 1.8, 0.8),
            },

            inventory: Vec::new(),
            hand: 0,
            mining_progress: None,
            interact_delay: 0.0,
            pulling: Vec::new(),
        },

        dropped_items: vec![],
        current_tick: 0,
    };


    game.player.add_item(Item { amount: 99, kind: items::ItemKind::Structure(StructureKind::Quarry)});
    game.player.add_item(Item { amount: 99, kind: items::ItemKind::Structure(StructureKind::Inserter)});

    let mut input = InputManager::default();


    for x in -RENDER_DISTANCE..RENDER_DISTANCE {
        for y in -RENDER_DISTANCE..RENDER_DISTANCE {
            for z in -RENDER_DISTANCE..RENDER_DISTANCE {
                let _= game.world.get_chunk(IVec3::new(x, y, z));
            }
        }
    }


    let mut renderer = Renderer::new((1920/2, 1080/2));
    let meshes = ItemMeshes::new();



    let block_outline_mesh = Mesh::from_obj("block_outline.obj");


    let fragment = Shader::new(&fs::read("fragment.fs").unwrap(), shader::ShaderType::Fragment).unwrap();
    let vertex = Shader::new(&fs::read("vertex.vs").unwrap(), shader::ShaderType::Vertex).unwrap();
    let world_shader = ShaderProgram::new(fragment, vertex).unwrap();


    renderer.window.set_cursor_mode(glfw::CursorMode::Disabled);

    let mut last_frame = 0.0;
    let mut time_since_last_simulation_step = 0.0;
    while !renderer.window.should_close() {
        let current_frame = renderer.glfw.get_time() as f64;

        let delta_time = (current_frame - last_frame) as f32;
        last_frame = current_frame;
        time_since_last_simulation_step += delta_time;


        // seperation for seperation sake
        process_events(&mut renderer, &mut input);


        // handle mouse movement 
        {
            let dt = input.mouse_delta();
            game.world.camera.yaw += dt.x * delta_time * MOUSE_SENSITIVITY;
            game.world.camera.pitch -= dt.y * delta_time * MOUSE_SENSITIVITY;
            
            game.world.camera.yaw = game.world.camera.yaw % 360f32.to_radians();

            game.world.camera.pitch = game.world.camera.pitch.clamp((-89.9f32).to_radians(), 89f32.to_radians()) % 360f32.to_radians();

            let yaw = game.world.camera.yaw;
            let pitch = game.world.camera.pitch;
            let x = yaw.cos() * pitch.cos();
            let y = pitch.sin();
            let z = yaw.sin() * pitch.cos();

            game.world.camera.direction = Vec3::new(x, y, z).normalize();
        }


        // handle keyboard input
        {
            let mut dir = Vec3::ZERO;
            if input.is_key_pressed(Key::W) {
                dir += game.world.camera.direction;
            } else if input.is_key_pressed(Key::S) {
                dir -= game.world.camera.direction;
            }

            if input.is_key_pressed(Key::D) {
                dir += game.world.camera.direction.cross(game.world.camera.up);
            } else if input.is_key_pressed(Key::A) {
                dir -= game.world.camera.direction.cross(game.world.camera.up);
            }

            dir.y = 0.0;
            let dir = dir.normalize_or_zero();
            let mov = dir * PLAYER_SPEED;
            game.player.body.velocity.x = mov.x;
            game.player.body.velocity.z = mov.z;


            if input.is_key_pressed(Key::Space) {
                game.player.body.velocity.y = 5.0;
            }

            if input.is_key_just_pressed(Key::P) {
                renderer.is_wireframe = !renderer.is_wireframe;
            }

            if input.is_key_just_pressed(Key::X) {
                game.player.hand += 1;
                game.player.hand %= game.player.inventory.len();
            }


        }


        // handle block interactions
        {
            game.player.interact_delay -= delta_time;

            'input_block: {
                if !input.is_button_pressed(MouseButton::Button1) {
                    game.player.mining_progress = None;
                    break 'input_block;
                }


                let Some((pos, _))= game.world.raycast_voxel(game.world.camera.position,
                                                  game.world.camera.direction,
                                                  PLAYER_REACH)
                else {
                    game.player.mining_progress = None;
                    break 'input_block;
                };


                let Some(mining_progress) = game.player.mining_progress
                else {
                    game.player.mining_progress = Some(0);
                    break 'input_block;
                };


                let voxel = game.world.get_voxel(pos);
                if mining_progress < voxel.kind.base_hardness() {
                    break 'input_block;
                }


                let item = game.break_block(pos);


                let dropped_item = DroppedItem {
                    item,
                    body: PhysicsBody {
                        position: (pos).as_vec3() + Vec3::new(0.5, 0.5, 0.5),
                        velocity: (random::<Vec3>() - Vec3::ONE*0.5) * 5.0,
                        aabb_dims: Vec3::splat(DROPPED_ITEM_SCALE),
                    },

                    creation_tick: game.current_tick,
                };

                game.dropped_items.push(dropped_item);
                game.player.mining_progress = None;
            }



            'input_block: {
                if input.is_button_pressed(MouseButton::Button2) && game.player.interact_delay < 0.0 {
                    let raycast = game.world.raycast_voxel(game.world.camera.position,
                                                      game.world.camera.direction,
                                                      PLAYER_REACH);
                    if let Some((pos, normal)) = raycast {
                        let voxel = game.world.get_voxel_mut(pos + normal);
                        if ! voxel.kind.is_air() {
                            break 'input_block;
                        }

                        let Some(item_in_hand) = game.player.inventory.get(game.player.hand)
                        else { break 'input_block };


                        if let Some(voxel) = item_in_hand.kind.as_voxel() {
                            let _ = game.player.take_item(game.player.hand, 1).unwrap();

                            game.world.get_voxel_mut(pos+normal).kind = voxel;
                            game.player.interact_delay = PLAYER_INTERACT_DELAY;
                        } else if let Some(structure_kind) = item_in_hand.kind.as_structure() {
                            let structure = Structure { position: pos+normal, data: StructureData::from_kind(structure_kind), direction: game.world.camera.compass_direction() };
                            if game.can_place_structure(&structure) {
                                let _ = game.player.take_item(game.player.hand, 1).unwrap();
                                game.add_structure(structure);
                                game.player.interact_delay = PLAYER_INTERACT_DELAY;
                            }
                        }
                    }
                }
            }
        }

        // simulate!
        {
            while time_since_last_simulation_step > DELTA_TICK {
                game.simulation_tick();
                time_since_last_simulation_step -= DELTA_TICK;
            }
        }


        // render
        {
            renderer.begin();

            // render the world
            {
                world_shader.use_program();
                
                let projection = glam::Mat4::perspective_rh_gl(80.0f32.to_radians(), 1920.0/1080.0, 0.01, 100_000.0);
                let view = game.world.camera.view_matrix();

                world_shader.set_matrix4(c"projection", projection);
                world_shader.set_matrix4(c"view", view);
                world_shader.set_vec4(c"modulate", Vec4::ONE);


                for (pos, chunk) in game.world.chunks.iter_mut() {
                    let pos = pos * CHUNK_SIZE as i32;
                    let mesh = chunk.mesh();
                    let model = Mat4::from_translation(pos.as_vec3());
                    world_shader.set_matrix4(c"model", model);

                    mesh.draw();

                }
            }



            // render items
            {
                for item in game.dropped_items.iter().chain(game.player.pulling.iter()) {
                    let position = item.body.position;
                    let mesh = meshes.get(item.item.kind);

                    let model = Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(DROPPED_ITEM_SCALE));
                    world_shader.set_matrix4(c"model", model);

                    mesh.draw();

                    let model = Mat4::from_translation(position + Vec3::Y);
                    world_shader.set_matrix4(c"model", model);
                }

            }


            // render structures
            {
                game.structures.for_each(|structure| {
                    structure.render(&renderer, &meshes, &world_shader);
                });
            }


            // render block outline
            {
                let raycast = game.world.raycast_voxel(game.world.camera.position,
                                                  game.world.camera.direction,
                                                  PLAYER_REACH);
                if let Some((pos, _)) = raycast {
                    let voxel = game.world.get_voxel(pos);
                    let target_hardness = voxel.kind.base_hardness();

                    let model = Mat4::from_translation(pos.as_vec3() + Vec3::new(0.5, -0.005, 0.5));
                    let model = model * Mat4::from_scale(Vec3::new(1.01, 1.01, 1.01));
                    world_shader.set_matrix4(c"model", model);

                    let modulate = if let Some(mining_progress) = game.player.mining_progress {
                        let progress = mining_progress as f32 / target_hardness as f32;
                        let eased = 1.0 - progress.powf(3.0);
                        (Vec4::ONE * eased).with_w(1.0)
                    } else {
                        Vec4::ONE
                    };

                    world_shader.set_vec4(c"modulate", modulate);
                    block_outline_mesh.draw();

                }
            }


            // render debug text
            {
                let mut text = String::new();

                let fps = (1.0 / delta_time).round();
                let colour_code = if fps > 55.0 { 'a' } else if fps > 25.0 { '6' } else { '4' };

                let _ = writeln!(text, "§eFPS: §{colour_code}{fps}§r");

                let _ = writeln!(text, "PITCH: §a{:.1} §rYAW: §a{:.1}§r", game.world.camera.pitch.to_degrees(), game.world.camera.yaw.to_degrees());
                let _ = writeln!(text, "POSITION: §a{:.1}, {:.1} {:.1}§r", game.world.camera.position.x, game.world.camera.position.y, game.world.camera.position.z);

                let chunk_pos = game.world.camera.position.as_ivec3().div_euclid(IVec3::splat(CHUNK_SIZE as i32));
                let _ = writeln!(text, "CHUNK POSITION: §a{}, {}, {}§r", chunk_pos.x, chunk_pos.y, chunk_pos.z);
                let _ = writeln!(text, "CHUNK COUNT: §a{}§r", game.world.chunks.len());
                let _ = writeln!(text, "DIRECTION: §b{:?}§r", game.world.camera.compass_direction());


                let target_block = game.world.raycast_voxel(game.world.camera.position, game.world.camera.direction, PLAYER_REACH);
                if let Some(target_block) = target_block {
                    let target_voxel = game.world.get_voxel(target_block.0);


                    let _ = writeln!(text, "TARGET LOCATION: {}", target_block.0);


                    let _ = write!(text, "TARGET BLOCK: ");


                    if target_voxel.kind.is_structure() {
                       let structure = game.structure_blocks.get(&target_block.0).unwrap();
                       let structure = &game.structures[structure.0];

                       let _ = writeln!(text, "STRUCTURE");
                       let _ = writeln!(text, "- POSITION: {}", structure.position);
                       let _ = writeln!(text, "- DIRECTION: {:?}", structure.direction);
                    } else {

                       let _ = writeln!(text, "{:?}", target_voxel.kind);
                    }


                    if let Some(mining_progress) = game.player.mining_progress {
                        let _ = writeln!(text, "MINING PROGRESS: {}/{}",
                                         mining_progress, target_voxel.kind.base_hardness());
                    }
                }


                if !game.work_queue.entries.is_empty() {
                    let mut cursor = game.work_queue.entries.lower_bound_mut(Bound::Unbounded);
                    let _ = writeln!(text, "WORK QUEUE:");

                    while let Some(((tick, id), ())) = cursor.next() {
                        let _ = writeln!(text, "- {:?} in {} ticks", game.structures[id.0].data.as_kind(), *tick - game.current_tick);
                    }
                }
                


                if !game.dropped_items.is_empty() {
                    let _ = writeln!(text, "DROPPED ITEMS:");


                    for dropped_item in game.dropped_items.iter() {
                        let _ = writeln!(text, "- {:?}: {:?}", dropped_item.item, dropped_item.body.position);
                    }

                }


                renderer.draw_text(&text, Vec2::ZERO, 1.4, Vec3::ONE);
            }


            // render inventory
            {
                let mut text = String::new();

                let _ = writeln!(text, "INVENTORY");
                let _ = writeln!(text, "HAND: {}", game.player.hand);
                for item in game.player.inventory.iter() {
                    let _ = writeln!(text, "{:?}", item);
                }

                // align it to the right of the screen
                let text = right_pad(&text);
                let text_size = renderer.text_size(&text, 0.4);

                let window_size = renderer.window.get_size();
                let window_size = Vec2::new(window_size.0 as f32, window_size.1 as f32);

                let pos = Vec2::new(window_size.x - text_size.x, 0.0);

                renderer.draw_text(&right_pad(&text), pos, 0.4, Vec3::ONE);

                renderer.draw_rect(window_size/2.0, Vec2::new(5.0, 5.0), Vec3::ONE);
            }

            renderer.end();
        }
    }
}


pub struct Game {
    world: World,
    player: Player,
    dropped_items: Vec<DroppedItem>,
    current_tick: u32,
    work_queue: WorkQueue,
    structures: KGenMap<StructureGen, StructureKey, Structure>,
    structure_blocks: HashMap<IVec3, StructureId>,
}


const TICKS_PER_SECOND : u32 = 60;
const DELTA_TICK : f32 = 1.0 / TICKS_PER_SECOND as f32; 


impl Game {
    fn can_place_structure(&mut self, structure: &Structure) -> bool {
        let blocks = structure.data.as_kind().blocks();
        for offset in blocks {
            if !self.world.get_voxel(structure.position + offset).kind.is_air() {
                return false;
            }
        }

        true
    }


    fn add_structure(&mut self, structure: Structure) {
        let id = self.structures.insert(structure);
        let id = StructureId(id);
        let structure = self.structures.get(id.0).unwrap();

        let placement_origin = structure.position - structure.data.as_kind().origin();

        let blocks = structure.data.as_kind().blocks();
        for offset in blocks {
            let pos = placement_origin + offset;
            self.world.get_voxel_mut(pos).kind = VoxelKind::StructureBlock;
            self.structure_blocks.insert(pos, id);
        }

        id.update(self);
    }


    fn break_block(&mut self, pos: IVec3) -> Item {
        let voxel = self.world.get_voxel_mut(pos);

        let item = if voxel.kind.is_structure() {
            let structure_id = *self.structure_blocks.get(&pos).unwrap();
            let structure = self.structures.remove(structure_id.0);
            let placement_origin = structure.position - structure.data.as_kind().origin();
            
            let blocks = structure.data.as_kind().blocks();
            let kind = structure.data.as_kind().item_kind();

            for offset in blocks {
                let pos = placement_origin + offset;

                self.world.get_voxel_mut(pos).kind = VoxelKind::Air;
                self.structure_blocks.remove(&pos).unwrap();
            }


            let mut cursor = self.work_queue.entries.lower_bound_mut(Bound::Unbounded);
            while let Some(((_, id), ())) = cursor.next() {
                if *id != structure_id { continue }

                cursor.remove_prev();
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


    fn simulation_tick(&mut self) {
        self.current_tick += 1;
        let delta_time = DELTA_TICK;

        if let Some(progress) = &mut self.player.mining_progress {
            *progress += 1;
        }


        // handle player physics
        {
            self.world.move_physics_body(delta_time, &mut self.player.body);

            self.world.camera.position = self.player.body.position;
            self.world.camera.position.y += 0.8;


            // iterate through the items in the world and
            // start pulling them if they are in distance
            // and they have been alive for more than 250ms
            {
                let mut i = 0;
                while let Some(item) = self.dropped_items.get(i) {
                    let lifetime = self.current_tick - item.creation_tick;
                    if lifetime < (0.2 * TICKS_PER_SECOND as f32) as u32 { i += 1; continue }

                    let distance = item.body.position.distance_squared(self.player.body.position);
                    if distance.abs() > 25.0 {
                        i += 1;
                        continue;
                    }

                    let item = self.dropped_items.remove(i);
                    self.player.pulling.push(item);

                }
            }


            // iterate through the items we are pulling
            // and collect them if they are in pickup area
            // else, pull them towards me
            {
                let mut i = 0;
                while let Some(item) = self.player.pulling.get_mut(i) {
                    let distance = item.body.position.distance_squared(self.player.body.position);

                    if distance.abs() < 0.5 {
                        let item = self.player.pulling.remove(i);
                        self.player.add_item(item.item);
                    } else {
                        item.body.position = item.body.position.move_towards(self.player.body.position, 10.0 * (1.0 + distance * 0.1) * delta_time);
                        i += 1;
                    }

                }
            }

        }

        // handle item physics
        {
            for item in self.dropped_items.iter_mut() {
                self.world.move_physics_body(delta_time, &mut item.body)
            }
        }


        // handle the work queue
        {
            let to_be_updated = self.work_queue.process(self.current_tick);
            for id in to_be_updated {
                if self.structures.get(id.1.0).is_some() {
                    id.1.update(self);
                }
            }
        }

    }
}


fn right_pad(str: &str) -> String {
    let Some(biggest_line) = str.lines().map(|x| x.len()).max()
    else { return String::new() };

    let mut string = String::with_capacity(biggest_line * str.lines().count());

    for line in str.lines() {
        if !string.is_empty() { string.push('\n'); }
        let _ = write!(string, "{}{}", " ".repeat(biggest_line - line.len()), line);
    }

    string
}


#[derive(Clone, Copy)]
pub struct PhysicsBody {
    position: Vec3,
    velocity: Vec3,

    aabb_dims: Vec3,
}


fn process_events(renderer: &mut Renderer,
                  input: &mut InputManager) {

    input.update();
 
    let events = glfw::flush_messages(&renderer.window_events).collect::<Vec<_>>();
    for event in events {
        match event.1 {
            glfw::WindowEvent::FramebufferSize(x, y) => {
                unsafe { gl::Viewport(0, 0, x, y); }
                renderer.resize();
            },


            glfw::WindowEvent::MouseButton(button, action, _) => {
                match action {
                    glfw::Action::Release => input.set_unpressed_button(button),
                    glfw::Action::Press => input.set_pressed_button(button),
                    glfw::Action::Repeat => (),
                }
            }


            glfw::WindowEvent::Key(key, _, action, _) => {
                match action {
                    glfw::Action::Release => input.set_unpressed_key(key),
                    glfw::Action::Press => input.set_pressed_key(key),
                    glfw::Action::Repeat => (),
                }
            }


            glfw::WindowEvent::CursorPos(x, y) => {
                input.move_cursor(Vec2::new(x as f32, y as f32));
            }


            _ => (),
        }
    }
}




impl World {
    fn raycast_voxel(&mut self, start: Vec3, dir: Vec3, max_dist: f32) -> Option<(IVec3, IVec3)> {
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


struct Player {
    body: PhysicsBody,
    inventory: Vec<Item>,
    hand: usize,
    mining_progress: Option<u32>,
    interact_delay: f32,
    pulling: Vec<DroppedItem>,
}


impl Player {
    pub fn add_item(&mut self, item: Item) {
        if let Some(slot) = self.inventory.iter_mut().find(|x| x.kind == item.kind) {
            slot.amount += item.amount;
        } else {
            self.inventory.push(item);
        }
    }


    pub fn take_item(&mut self, index: usize, amount: u32) -> Option<Item> {
        let slot = self.inventory.get_mut(index)?;

        if slot.amount < amount {
            return None;
        }


        slot.amount -= amount;
        let slot = *slot;
        if slot.amount == 0 {
            self.inventory.remove(index);

            if !self.inventory.is_empty() {
                self.hand = self.hand % self.inventory.len();
            }
        }


        Some(Item { amount, kind: slot.kind })
    }
}

struct Camera {
    position: Vec3,
    direction: Vec3,
    up: Vec3,


    pitch: f32,
    yaw: f32,

}


#[derive(Clone, PartialEq, Debug)]
pub struct AABB {
    w: f32,
    h: f32,
    d: f32,
}


impl AABB {
    pub fn new(w: f32, h: f32, d: f32) -> Self {
        Self {
            w,
            h,
            d,
        }
    }

}



impl Camera {
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_to_rh(self.position, self.direction, self.up)
    }


    pub fn compass_direction(&self) -> CompassDirection {
        let mut angle = self.yaw % TAU;

        if angle < 0.0 { angle += TAU }

        let angle = angle;
        let sector = (angle / (PI/2.0)).round() as i32 % 4;

        match sector {
            0 => CompassDirection::South,
            1 => CompassDirection::West,
            2 => CompassDirection::North,
            3 => CompassDirection::East,
            _ => unreachable!(),
        }
    }
}



struct World {
    chunks: HashMap<IVec3, Chunk>,
    camera: Camera,
}


impl World {
    pub fn get_chunk(&mut self, pos: IVec3) -> &mut Chunk {
        if !self.chunks.contains_key(&pos) {
            self.chunks.insert(pos, Chunk::generate(pos));
        }

        self.chunks.get_mut(&pos).unwrap()
    }


    pub fn get_voxel(&mut self, pos: IVec3) -> &Voxel {
        let chunk_pos = pos.div_euclid(IVec3::splat(CHUNK_SIZE as i32));
        let chunk_local_pos = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));

        self.get_chunk(chunk_pos).get(chunk_local_pos)
    }


    pub fn get_voxel_mut(&mut self, pos: IVec3) -> &mut Voxel {
        let chunk_pos = pos.div_euclid(IVec3::splat(CHUNK_SIZE as i32));
        let chunk_local_pos = pos.rem_euclid(IVec3::splat(CHUNK_SIZE as i32));

        self.get_chunk(chunk_pos).get_mut(chunk_local_pos)
    }
}


