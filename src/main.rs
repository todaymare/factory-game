#![feature(duration_millis_float)]
#![feature(portable_simd)]
#![feature(btree_cursors)]
#![feature(str_as_str)]
#![feature(path_add_extension)]

pub mod shader;
pub mod mesh;
pub mod quad;
pub mod renderer;
pub mod input;
pub mod items;
pub mod structures;
pub mod gen_map;
pub mod voxel_world;
pub mod directions;
pub mod ui;
pub mod save_system;
pub mod commands;

use core::{f32, time};
use std::{char, collections::{HashMap, HashSet}, env, f32::consts::{PI, TAU}, fmt::{Display, Write}, fs, io::BufReader, ops::{self, Bound}, simd::f32x4, time::Instant};

use commands::{Command, CommandRegistry};
use directions::CardinalDirection;
use save_format::Value;
use ui::UILayer;
use voxel_world::{chunk::{Chunk, MeshState, CHUNK_SIZE}, split_world_pos, voxel::{Voxel, VoxelKind}, VoxelWorld};
use glam::{IVec3, Mat4, Vec2, Vec3, Vec4};
use glfw::{GlfwReceiver, Key, MouseButton, WindowEvent};
use input::InputManager;
use items::{DroppedItem, Item, ItemKind, Assets};
use mesh::{Mesh, Vertex};
use rand::{random, seq::IndexedRandom};
use renderer::Renderer;
use shader::{Shader, ShaderProgram};
use sti::{arena::Arena, define_key, format_in, key::Key as _, vec::KVec};
use structures::{belts::SccId, strct::{rotate_block_vector, InserterState, Structure, StructureData, StructureKind}, Slot, StructureId, Structures};

define_key!(EntityId(u32));


const MOUSE_SENSITIVITY : f32 = 0.1;

const PLAYER_REACH : f32 = 5.0;
const PLAYER_SPEED : f32 = 5.0;
const PLAYER_PULL_DISTANCE : f32 = 3.5;
const PLAYER_INTERACT_DELAY : f32 = 0.2;

const RENDER_DISTANCE : i32 = 4;

const DROPPED_ITEM_SCALE : f32 = 0.25;


fn main() {
    let mut game = Game::new();

    let mut input = InputManager::default();
    let mut renderer = Renderer::new((1920/2, 1080/2));
    let mut ui_layer = UILayer::Gameplay;

    for x in -RENDER_DISTANCE..RENDER_DISTANCE {
        for y in -RENDER_DISTANCE..RENDER_DISTANCE {
            for z in -RENDER_DISTANCE..RENDER_DISTANCE {
                let _= game.world.get_chunk(IVec3::new(x, y, z));
            }
        }
    }

    let block_outline_mesh = Mesh::from_obj("assets/models/block_outline.obj");


    let fragment = Shader::new(&fs::read("fragment.fs").unwrap(), shader::ShaderType::Fragment).unwrap();
    let vertex = Shader::new(&fs::read("vertex.vs").unwrap(), shader::ShaderType::Vertex).unwrap();
    let world_shader = ShaderProgram::new(fragment, vertex).unwrap();


    renderer.window.set_cursor_mode(glfw::CursorMode::Disabled);


    if !fs::exists("saves/").is_ok_and(|f| f == true) {
        let _ = fs::create_dir("saves/");
        let _ = fs::create_dir("saves/chunks/");
        game.save();
    }

    game.load();

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
        if matches!(ui_layer, UILayer::Gameplay) {
            let dt = input.mouse_delta();
            game.camera.yaw += dt.x * delta_time * MOUSE_SENSITIVITY;
            game.camera.pitch -= dt.y * delta_time * MOUSE_SENSITIVITY;
            
            game.camera.yaw = game.camera.yaw % 360f32.to_radians();

            game.camera.pitch = game.camera.pitch.clamp((-89.9f32).to_radians(), 89f32.to_radians()) % 360f32.to_radians();

            let yaw = game.camera.yaw;
            let pitch = game.camera.pitch;
            let x = yaw.cos() * pitch.cos();
            let y = pitch.sin();
            let z = yaw.sin() * pitch.cos();

            game.camera.direction = Vec3::new(x, y, z).normalize();


            let dt = input.scroll_delta();
            if dt.y > 0.0 { game.player.hand += 1 }
            if dt.y < 0.0 && game.player.hand == 0 { game.player.hand = game.player.inventory.len() - 1 }
            else if dt.y < 0.0 { game.player.hand -= 1 }

            game.player.hand %= game.player.inventory.len();
        }


        // handle keyboard input
        'input: {
            if !matches!(ui_layer, UILayer::Gameplay) {
                break 'input;
            }

            let mut dir = Vec3::ZERO;
            if input.is_key_pressed(Key::W) {
                dir += game.camera.direction;
            } else if input.is_key_pressed(Key::S) {
                dir -= game.camera.direction;
            }

            if input.is_key_pressed(Key::D) {
                dir += game.camera.direction.cross(game.camera.up);
            } else if input.is_key_pressed(Key::A) {
                dir -= game.camera.direction.cross(game.camera.up);
            }

            dir.y = 0.0;
            let dir = dir.normalize_or_zero();
            let mov = dir * game.player.speed;
            game.player.body.velocity.x = mov.x;
            game.player.body.velocity.z = mov.z;


            if input.is_key_pressed(Key::Space) {
                game.player.body.velocity.y = 5.0;
            }


            if input.is_key_pressed(Key::Q) {
                let raycast = game.world.raycast_voxel(game.camera.position,
                                                  game.camera.direction,
                                                  PLAYER_REACH);
                if let Some((pos, _)) = raycast {
                    let voxel = game.world.get_voxel(pos);
                    if voxel.kind.is_structure() {
                        let structure = game.world.structure_blocks.get(&pos).unwrap();
                        let structure = game.structures.get_mut(*structure);

                        if let StructureData::Inserter { filter, .. } = &mut structure.data {
                            *filter = None; 
                        }
                        else {
                            for index in 0..structure.available_items_len() {
                                let item = structure.try_take(index);
                                if let Some(item) = item {
                                    let dropped_item = DroppedItem::new(item, pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5));
                                    game.world.dropped_items.push(dropped_item);
                                    break;
                                }

                            }
                        }
                    }
                }
            }


            if input.is_key_pressed(Key::R) {
                let raycast = game.world.raycast_voxel(game.camera.position,
                                                  game.camera.direction,
                                                  PLAYER_REACH);
                if let Some((pos, _)) = raycast {
                    let voxel = game.world.get_voxel(pos);
                    if voxel.kind.is_structure() {
                        let structure = game.world.structure_blocks.get(&pos).unwrap();
                        let structure = game.structures.get_mut(*structure);

                        let item = game.player.take_item(game.player.hand, 1);

                        if let StructureData::Inserter { filter, .. } = &mut structure.data {
                            *filter = item.map(|x| x.kind);
                            if let Some(item) = item {
                                game.player.add_item(item);
                            }
                        }

                        else if let Some(item) = item {
                            if structure.can_accept(item) {
                                structure.give_item(item);
                            } else {
                                let dropped_item = DroppedItem::new(item, pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5));
                                game.world.dropped_items.push(dropped_item);
                            }
                        }

                    }
                }
            }


            if input.is_key_just_pressed(Key::E) {
                if matches!(ui_layer, UILayer::Inventory { .. }) {
                    ui_layer = UILayer::Gameplay
                } else {
                    ui_layer = UILayer::Inventory { just_opened: true }
                }
            }


            if input.is_key_just_pressed(Key::G) {
                let belts = game.structures.belts(&game.world);
                fs::write("sscs.dot", belts.scc_graph().as_bytes()).unwrap();
            }


            if input.is_key_just_pressed(Key::P) {
                renderer.is_wireframe = !renderer.is_wireframe;
            }


            if input.is_key_just_pressed(Key::Enter) {
                if matches!(ui_layer, UILayer::Console { .. }) {
                    ui_layer = UILayer::Gameplay
                } else {
                    ui_layer = UILayer::Console { text: String::new(), backspace_cooldown: 1.0, timer: 0.0, cursor: 0, just_opened: true, offset: 1 }
                }
            }


            if input.is_key_just_pressed(Key::F6) {
                println!("saving");
                let time = Instant::now();
                game.save();
                println!("saved in {}ms", time.elapsed().as_millis());
            }


            if input.is_key_pressed(Key::F3) && input.is_key_just_pressed(Key::T) {
                game.world.chunks.iter_mut().for_each(|x| x.1.mesh_state = MeshState::ShouldUpdate);
            }



            if input.is_key_just_pressed(Key::F7) {
                println!("loading");
                let time = Instant::now();
                game.load();
                println!("loaded save in {}ms", time.elapsed().as_millis());
            }



            if input.is_key_just_pressed(Key::Num1) { game.player.hand = 0 }
            if input.is_key_just_pressed(Key::Num2) { game.player.hand = 1 }
            if input.is_key_just_pressed(Key::Num3) { game.player.hand = 2 }
            if input.is_key_just_pressed(Key::Num4) { game.player.hand = 3 }
            if input.is_key_just_pressed(Key::Num5) { game.player.hand = 4 }
            if input.is_key_just_pressed(Key::Num6) { game.player.hand = 5 }
        }


        // handle block interactions
        'outer: {
            game.player.interact_delay -= delta_time;


            if !matches!(ui_layer, UILayer::Gameplay) {
                break 'outer;
            }


            'input_block: {
                if !input.is_button_pressed(MouseButton::Button1) {
                    game.player.mining_progress = None;
                    break 'input_block;
                }


                let Some((pos, _))= game.world.raycast_voxel(game.camera.position,
                                                             game.camera.direction,
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


                let item = game.world.break_block(&mut game.structures, pos);


                let dropped_item = DroppedItem::new(item, pos.as_vec3() + Vec3::new(0.5, 0.5, 0.5));

                game.world.dropped_items.push(dropped_item);
                game.player.mining_progress = None;
            }



            'input_block: {
                if input.is_button_just_pressed(MouseButton::Button2) {
                    game.player.interact_delay = 0.0;
                }

                if game.player.interact_delay > 0.0 {
                    break 'input_block;
                }

                if !input.is_button_pressed(MouseButton::Button2) {
                    break 'input_block;
                }


                let Some((pos, normal)) = game.world.raycast_voxel(game.camera.position,
                                                                   game.camera.direction,
                                                                   PLAYER_REACH)
                else { break 'input_block };

                let place_position = pos + normal;

                let voxel = game.world.get_voxel(place_position);
                if !voxel.kind.is_air() { break 'input_block }

                let Some(Some(item_in_hand)) = game.player.inventory.get(game.player.hand)
                else { break 'input_block };

                if let Some(voxel) = item_in_hand.kind.as_voxel() {
                    let _ = game.player.take_item(game.player.hand, 1).unwrap();

                    game.world.get_voxel_mut(place_position).kind = voxel;

                } else if let Some(structure_kind) = item_in_hand.kind.as_structure() {

                    if !game.can_place_structure(structure_kind, place_position, game.camera.compass_direction()) {
                        break 'input_block;
                    }

                    let structure = Structure::from_kind(structure_kind, pos+normal, game.camera.compass_direction());
                    let _ = game.player.take_item(game.player.hand, 1).unwrap();
                    game.structures.add_structure(&mut game.world, structure);
                }


                game.player.interact_delay = PLAYER_INTERACT_DELAY;

            }
        }

        // simulate!
        {
            while time_since_last_simulation_step > DELTA_TICK {
                game.simulation_tick();
                time_since_last_simulation_step -= DELTA_TICK;
            }

        }


        game.world.process();


        // render
        {
            renderer.begin();

            // render the world
            {
                world_shader.use_program();
                
                let projection = glam::Mat4::perspective_rh_gl(80.0f32.to_radians(), 1920.0/1080.0, 0.01, 100_000.0);
                let view = game.camera.view_matrix();

                world_shader.set_matrix4(c"projection", projection);
                world_shader.set_matrix4(c"view", view);
                world_shader.set_vec4(c"modulate", Vec4::ONE);

                let (player_chunk, _) = split_world_pos(game.player.body.position.as_ivec3());

                for x in -RENDER_DISTANCE..RENDER_DISTANCE {
                    for y in -RENDER_DISTANCE..RENDER_DISTANCE {
                        for z in -RENDER_DISTANCE..RENDER_DISTANCE {
                            let pos = player_chunk + IVec3::new(x, y, z);

                            let mesh = game.world.get_mesh(pos);

                            let offset = pos * IVec3::splat(CHUNK_SIZE as i32);
                            let model = Mat4::from_translation(offset.as_vec3());
                            world_shader.set_matrix4(c"model", model);

                            mesh.draw();
                        }
                    }
                }
            }



            // render items
            {
                for item in game.world.dropped_items.iter().chain(game.player.pulling.iter()) {
                    let position = item.body.position;

                    let scale = Vec3::splat(DROPPED_ITEM_SCALE);
                    let rot = (game.current_tick - item.creation_tick).u32() as f32 / TICKS_PER_SECOND as f32;

                    renderer.draw_item(&world_shader, item.item.kind, position, scale, rot);
                }

            }


            // render structures
            {
                game.structures.for_each(|structure| {
                    structure.render(&game.structures, &renderer, &world_shader);
                });
            }


            // render block outline
            {
                let raycast = game.world.raycast_voxel(game.camera.position,
                                                  game.camera.direction,
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


            // render ui
            ui_layer.render(&mut game, &input, &mut renderer, delta_time);

            let current_cm = renderer.window.get_cursor_mode();
            let cm = ui_layer.capture_mode();
            if current_cm != cm {
                renderer.window.set_cursor_mode(cm);

                let window = renderer.window.get_size();
                renderer.window.set_cursor_pos(window.0 as f64 / 2.0,
                                               window.1 as f64 / 2.0);
            }


            // render hotbar
            {
                let window = renderer.window_size();
                let bottom_centre = Vec2::new(window.x * 0.5, window.y);

                let slot_size = 64.0;
                let slot_amount = game.player.inventory.len();
                let padding = 16.0;

                let mut base = bottom_centre - Vec2::new((padding + slot_size) * slot_amount as f32 * 0.5, slot_size + padding);

                for (i, slot) in game.player.inventory.iter().enumerate() {
                    let colour = if i == game.player.hand { Vec4::new(1.0, 0.0, 0.0, 1.0) }
                                 else { (Vec4::ONE * 0.2).with_w(1.0) };
                    renderer.draw_rect(base, Vec2::splat(slot_size), colour);
                    if let Some(item) = slot {
                        renderer.draw_item_icon(item.kind, base+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
                        renderer.draw_text(format!("{}", item.amount).as_str(), base+slot_size*0.05, 0.5, Vec4::ONE);
                    }
                    base += Vec2::new(slot_size+padding, 0.0);
                }

            }

            renderer.end();
        }
    }


    game.save();
}


pub struct Game {
    world: VoxelWorld,
    camera: Camera,
    player: Player,
    current_tick: Tick,
    structures: Structures,
    command_registry: CommandRegistry,
}


const TICKS_PER_SECOND : u32 = 60;
const DELTA_TICK : f32 = 1.0 / TICKS_PER_SECOND as f32; 


impl Game {
    pub fn new() -> Game {
        let mut this = Game {
            world: VoxelWorld::new(),
            structures: Structures::new(),

            camera: Camera {
                position: Vec3::ZERO,
                direction: Vec3::Z,
                up: Vec3::new(0.0, 1.0, 0.0),
                pitch: 0.0,
                yaw: 90.0f32.to_radians(),

            },



            player: Player {
                body: PhysicsBody {
                    position: Vec3::new(0.0, 10.0, 0.0),
                    velocity: Vec3::ZERO,
                    aabb_dims: Vec3::new(0.8, 1.8, 0.8),
                },

                inventory: [None; 6],
                hand: 0,
                mining_progress: None,
                interact_delay: 0.0,
                pulling: Vec::new(),
                speed: PLAYER_SPEED,
            },

            current_tick: Tick::initial(),
            command_registry: CommandRegistry::new(),
        };


        this.command_registry.register("speed", |game, cmd| {
            let speed = cmd.arg(0)?.as_f32()?;
            game.player.speed = speed;
            Some(())
        });

        this.command_registry.register("give", |game, cmd| {
            let item = cmd.arg(0)?.as_str();
            let kind = *ItemKind::ALL.iter().find(|x| x.to_string() == item)?;

            let amount = cmd.arg(1)?.as_u32()?;

            let item = Item { amount, kind };
            game.world.dropped_items.push(DroppedItem::new(item, game.player.body.position));

            Some(())
        });

        this.command_registry.register("tp", |game, cmd| {
            let x = cmd.arg(0)?.as_f32()?;
            let y = cmd.arg(1)?.as_f32()?;
            let z = cmd.arg(2)?.as_f32()?;
            let pos = Vec3::new(x, y, z);
            game.player.body.position = pos;

            Some(())
        });

        this.command_registry.register("clear", |game, _| {
            game.player.inventory.iter_mut().for_each(|x| *x = None);

            Some(())
        });


        this
    }


    fn call_command(&mut self, command: Command) {
        let Some(func) = self.command_registry.find(command.command())
        else {
            self.command_registry.previous_commands.push(command);
            return;
        };

        func(self, &command);

        self.command_registry.previous_commands.push(command);
    }

    
    fn can_place_structure(&mut self, structure: StructureKind, pos: IVec3, direction: CardinalDirection) -> bool {
        let pos = pos - structure.origin(direction);
        let blocks = structure.blocks(direction);
        for offset in blocks {
            if !self.world.get_voxel(pos + offset).kind.is_air() {
                return false;
            }
        }

        true
    }


    fn simulation_tick(&mut self) {
        self.current_tick = self.current_tick.inc();

        let delta_time = DELTA_TICK;

        if let Some(progress) = &mut self.player.mining_progress {
            *progress += 1;
        }


        // handle player physics
        {
            self.world.move_physics_body(delta_time, &mut self.player.body);

            self.camera.position = self.player.body.position;
            self.camera.position.y += 0.8;


            // iterate through the items in the world and
            // start pulling them if they are in distance
            // and they have been alive for more than 250ms
            {
                let mut i = 0;
                while let Some(item) = self.world.dropped_items.get(i) {
                    let lifetime = self.current_tick - item.creation_tick;
                    if item.creation_tick == Tick::NEVER {
                        self.world.dropped_items[i].creation_tick = self.current_tick;
                        i += 1;
                        continue;
                    }

                    if lifetime.u32() < (0.2 * TICKS_PER_SECOND as f32) as u32 { i += 1; continue }

                    let distance = item.body.position.distance_squared(self.player.body.position);
                    if distance.abs() > PLAYER_PULL_DISTANCE*PLAYER_PULL_DISTANCE {
                        i += 1;
                        continue;
                    }

                    if !self.player.can_give(item.item) { i += 1; continue };

                    let item = self.world.dropped_items.remove(i);
                    self.player.pulling.push(item);

                }
            }


            // iterate through the items we are pulling
            // and collect them if they are in pickup area
            // else, pull them towards me
            {
                let mut i = 0;
                let mut pulling = core::mem::take(&mut self.player.pulling);
                while let Some(item) = pulling.get_mut(i) {
                    let distance = item.body.position.distance_squared(self.player.body.position);

                    let can_give = self.player.can_give(item.item);
                    if !can_give {
                        let item = pulling.remove(i);
                        self.world.dropped_items.push(item);
                        continue;
                    }

                    if distance.abs() < 0.5 {
                        let item = pulling.remove(i);
                        self.player.add_item(item.item);
                    } else {
                        item.body.position = item.body.position.move_towards(self.player.body.position, 10.0 * (1.0 + distance * 0.1) * delta_time);
                        i += 1;
                    }
                }

                self.player.pulling = pulling;

            }

        }

        // handle item physics
        {
            let mut dropped_items = core::mem::take(&mut self.world.dropped_items);
            for item in dropped_items.iter_mut() {
                self.world.move_physics_body(delta_time, &mut item.body)
            }

            self.world.dropped_items = dropped_items;
        }


        self.structures.process(&mut self.world);
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
            },


            glfw::WindowEvent::MouseButton(button, action, _) => {
                match action {
                    glfw::Action::Release => input.set_unpressed_button(button),
                    glfw::Action::Press => input.set_pressed_button(button),
                    glfw::Action::Repeat => input.set_pressed_button(button),
                }
            }


            glfw::WindowEvent::Key(key, _, action, _) => {
                match action {
                    glfw::Action::Release => input.set_unpressed_key(key),
                    glfw::Action::Press => input.set_pressed_key(key),
                    glfw::Action::Repeat => (),
                }
            }


            glfw::WindowEvent::Scroll(x, y) => {
                input.scroll(Vec2::new(x as f32, y as f32));
            }


            glfw::WindowEvent::CursorPos(x, y) => {
                input.move_cursor(Vec2::new(x as f32, y as f32));
            }


            glfw::WindowEvent::Char(ch) => {
                input.new_char(ch);
            }


            _ => (),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct Tick(u32);


impl Tick {
    pub const NEVER : Tick = Tick(0);


    pub fn new(num: u32) -> Self {
        Self(num)
    }

    
    pub fn initial() -> Self { Self::new(1) }


    pub fn inc(mut self) -> Self {
        self.0 += 1;
        self
    }


    pub fn elapsed_since(self, initial: Tick) -> Tick {
        Tick(initial.0 - self.0)
    }


    pub fn u32(self) -> u32 { self.0 }

}


impl ops::Add for Tick {
    type Output = Tick;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}


impl ops::Sub for Tick {
    type Output = Tick;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}


struct Player {
    body: PhysicsBody,
    inventory: [Option<Item>; 6],
    hand: usize,
    mining_progress: Option<u32>,
    interact_delay: f32,
    pulling: Vec<DroppedItem>,
    speed: f32,
}


impl Player {
    pub fn can_give(&self, item: Item) -> bool {
        for slot in &self.inventory {
            let Some(inv_item) = slot
            else { continue };

            if inv_item.kind != item.kind { continue }
            return true;
        }


        for slot in &self.inventory {
            if slot.is_some() { continue }

            return true;
        }

        false
    }


    pub fn add_item(&mut self, item: Item) {
        assert!(self.can_give(item));
        for slot in &mut self.inventory {
            let Some(inv_item) = slot
            else { continue };

            if inv_item.kind != item.kind { continue }

            inv_item.amount += item.amount;
            return;
        }


        for slot in &mut self.inventory {
            if slot.is_some() { continue }

            *slot = Some(item);
            return;
        }
    }


    pub fn take_item(&mut self, index: usize, amount: u32) -> Option<Item> {
        let slot = self.inventory.get_mut(index)?.as_mut()?;


        if slot.amount < amount {
            return None;
        }


        slot.amount -= amount;
        let slot = *slot;
        if slot.amount == 0 {
            self.inventory[index] = None;

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


impl Camera {
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_to_rh(self.position, self.direction, self.up)
    }


    pub fn compass_direction(&self) -> CardinalDirection {
        let mut angle = self.yaw % TAU;

        if angle < 0.0 { angle += TAU }

        let angle = angle;
        let sector = (angle / (PI/2.0)).round() as i32 % 4;

        match sector {
            0 => CardinalDirection::South,
            1 => CardinalDirection::West,
            2 => CardinalDirection::North,
            3 => CardinalDirection::East,
            _ => unreachable!(),
        }
    }
}


fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_ = h / 60.0;
    let x = c * (1.0 - (h_ % 2.0 - 1.0).abs());
    
    let (r1, g1, b1) = match h_ as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 | _ => (c, 0.0, x),
    };

    let m = l - c / 2.0;
    let (r, g, b) = (r1 + m, g1 + m, b1 + m);

    let to_255 = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;

    (to_255(r), to_255(g), to_255(b))
}

fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

// Usage
fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let (r, g, b) = hsl_to_rgb(h, s, l);
    rgb_to_hex(r, g, b)
}


