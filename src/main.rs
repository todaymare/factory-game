#![feature(portable_simd)]
#![feature(btree_cursors)]
#![feature(str_as_str)]
#![feature(path_add_extension)]
#![feature(if_let_guard)]
#![feature(generic_arg_infer)]
#![feature(iter_array_chunks)]
#![feature(seek_stream_len)]

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
pub mod crafting;
pub mod perlin;
pub mod frustum;
pub mod game;
pub mod constants;
pub mod buddy_allocator;

use std::{f32::consts::{PI, TAU}, fs, hash::Hash, ops::{self}, time::Instant};

use commands::{Command, CommandRegistry};
use constants::CHUNK_SIZE;
use directions::{CardinalDirection, Direction};
use frustum::Frustum;
use game::Game;
use rand::seq::IndexedRandom;
use tracing::{error, info, trace, warn, Level};
use ui::{InventoryMode, UILayer, HOTBAR_KEYS};
use voxel_world::{mesh::ChunkVertex, split_world_pos, voxel::Voxel, VoxelWorld};
use glam::{DVec2, DVec3, IVec2, IVec3, Mat4, Quat, Vec2, Vec3, Vec4, Vec4Swizzles};
use input::InputManager;
use items::{DroppedItem, Item, ItemKind};
use mesh::Mesh;
use renderer::{gpu_allocator::GPUAllocator, DepthBuffer, Renderer, VoxelShaderUniform};
use shader::{Shader, ShaderProgram};
use sti::{define_key, hash::fxhash::FxHasher32};
use structures::{strct::{Structure, StructureData, StructureKind}, Structures};
use wgpu::util::RenderEncoder;
use winit::{dpi::{LogicalPosition, PhysicalPosition, PhysicalSize}, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, platform::macos::EventLoopBuilderExtMacOS, window::{CursorGrabMode, Window, WindowButtons, WindowId}};
use winit::application::ApplicationHandler;



define_key!(EntityId(u32));


const MOUSE_SENSITIVITY : f32 = 0.0016;

const PLAYER_REACH : f32 = 5.0;
const PLAYER_SPEED : f32 = 10.0;
const PLAYER_PULL_DISTANCE : f32 = 3.5;
const PLAYER_INTERACT_DELAY : f32 = 0.125;
const PLAYER_HOTBAR_SIZE : usize = 5;
const PLAYER_ROW_SIZE : usize = 6;
const PLAYER_INVENTORY_SIZE : usize = PLAYER_ROW_SIZE * PLAYER_HOTBAR_SIZE;

const RENDER_DISTANCE : i32 = 48;

const DROPPED_ITEM_SCALE : f32 = 0.5;

const TICKS_PER_SECOND : u32 = 60;
const DELTA_TICK : f32 = 1.0 / TICKS_PER_SECOND as f32; 

struct App {
    window: Option<Window>,
    renderer: Option<Renderer>,
    last_frame: Instant,
    time_since_last_simulation: f32,
    game: Game,
    input: InputManager,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(event_loop.create_window(Window::default_attributes()).unwrap());
        let window = self.window.as_ref().unwrap();

        window.set_cursor_visible(false);
        window.set_cursor_grab(CursorGrabMode::Confined) // or Locked
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked))
            .unwrap();
        let static_window = unsafe { core::mem::transmute::<&Window, &'static Window>(window) };
        self.renderer = Some(pollster::block_on(Renderer::new(static_window)));
    }


    fn device_event(
            &mut self,
            _: &ActiveEventLoop,
            _: winit::event::DeviceId,
            event: winit::event::DeviceEvent,
        ) {
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                let delta = Vec2::new(delta.0 as f32, delta.1 as f32);
                self.input.move_cursor(self.input.mouse_position() - delta);
            },
            _ => (),
        }
    }


    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },


            WindowEvent::MouseWheel { delta, .. } => {
                let vec = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => Vec2::new(x, y),
                    winit::event::MouseScrollDelta::PixelDelta(pp) => DVec2::new(pp.x, pp.y).as_vec2(),
                };

                self.input.scroll(vec);
            }


            WindowEvent::MouseInput { state, button, .. } => {
                match state {
                    winit::event::ElementState::Pressed => self.input.set_pressed_button(button),
                    winit::event::ElementState::Released => self.input.set_unpressed_button(button),
                }
            }

            WindowEvent::CursorMoved { position: pos, .. } => {
                self.input.move_cursor(DVec2::new(pos.x, pos.y).as_vec2());
            }


            WindowEvent::KeyboardInput { event, .. } => {
                match event.state {
                    winit::event::ElementState::Pressed => self.input.set_pressed_key(event.physical_key),
                    winit::event::ElementState::Released => self.input.set_unpressed_key(event.physical_key),
                }
            }


            WindowEvent::RedrawRequested => {
                let game = &mut self.game;
                let Some(renderer) = &mut self.renderer
                else { error!("redraw-requested: no renderer found"); return; };


                let now = Instant::now();
                let dt = now.duration_since(self.last_frame).as_secs_f32();
                self.last_frame = now;

                self.time_since_last_simulation += dt;

                game.handle_input(dt, &mut self.input);
                self.input.update();
                if !game.camera.front.is_normalized() { panic!("{:?}", self.game.camera.front); }

                while self.time_since_last_simulation > game.settings.delta_tick {
                    game.simulation_tick();
                    self.time_since_last_simulation -= game.settings.delta_tick;
                }

                game.update_world(&mut renderer.voxel_pipeline.vertex_buf, &mut renderer.voxel_pipeline.index_buf);


                let output = renderer.surface.get_current_texture().unwrap();
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("voxel-command-encoder"),
                });


                game.world.process_meshes(
                    &renderer.device,
                    &mut encoder,
                    &mut renderer.staging_buffer,
                    &mut renderer.voxel_pipeline.vertex_buf,
                    &mut renderer.voxel_pipeline.index_buf
                );


                let simulation_time = now.elapsed();
                let now = Instant::now();

                let c = game.sky_colour.as_dvec4();
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("voxel-render-pass"),
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color { r: c.x, g: c.y, b: c.z, a: c.w }),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                    ],

                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &renderer.voxel_pipeline.depth_buffer.view, // from Step 1
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0), // farthest
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),

                    ..Default::default()
                });

                let camera = game.camera.position;
                let projection = game.camera.perspective_matrix();
                let view = game.camera.view_matrix();

                // render chunks
                {
                    let voxel_pipeline = &mut renderer.voxel_pipeline;
                    let rd = game.settings.render_distance;
                    let fog_distance = (rd - 1) as f32;

                    let uniform = VoxelShaderUniform {
                        view,
                        projection,
                        modulate: Vec4::ONE,
                        camera_pos: camera.as_vec3(),
                        fog_color: game.sky_colour.xyz(),
                        fog_density: 1.0,
                        fog_start: fog_distance * CHUNK_SIZE as f32 * 0.9,
                        fog_end: fog_distance * CHUNK_SIZE as f32,
                        pad_00: 0.0,
                        pad_01: 0.0,
                        pad_02: 0.0,


                    };

                    pass.set_pipeline(&voxel_pipeline.pipeline);
                    voxel_pipeline.frame_uniform.update(&renderer.queue, &uniform);
                    voxel_pipeline.frame_uniform.use_uniform(&mut pass);
                    pass.set_bind_group(1, voxel_pipeline.model_uniform.bind_group(), &[]);

                    pass.set_vertex_buffer(0, voxel_pipeline.vertex_buf.ssbo.buffer.slice(..));
                    pass.set_index_buffer(voxel_pipeline.index_buf.ssbo.buffer.slice(..), wgpu::IndexFormat::Uint32);

                    let frustum = match &game.lock_frustum {
                        Some(f) => f.clone(),
                        None => Frustum::compute(projection, view),
                    };

                            
                    let (player_chunk, _) = split_world_pos(game.player.body.position.as_ivec3());

                    let mut offsets = vec![];
                    for y in -rd..rd {
                        for z in -rd..rd {
                            for x in -rd..rd {
                                let offset = IVec3::new(x, y, z);

                                if offset.length_squared() > (rd*rd) {
                                    continue;
                                }


                                let chunk_pos = player_chunk + offset;

                                let min = chunk_pos * CHUNK_SIZE as i32;
                                let max = (chunk_pos + IVec3::ONE) * CHUNK_SIZE as i32;

                                let min = (min.as_dvec3() - camera).as_vec3();
                                let max = (max.as_dvec3() - camera).as_vec3();

                                if !frustum.is_box_visible(min, max) {
                                    continue;
                                }

                                let Some(meshes) = game.world.try_get_mesh(chunk_pos)
                                else { continue };

                                let dir_from_camera = offset.as_vec3().normalize();


                                let offset = chunk_pos * IVec3::splat(CHUNK_SIZE as i32);
                                let offset = offset.as_dvec3() - camera;

                                let offset = offset.as_vec3();
                                offsets.push(Vec4::new(offset.x, offset.y, offset.z, 0.0));

                                let mut did_draw = false;
                                for (i, mesh) in meshes.iter().enumerate() {
                                    let Some(mesh) = mesh
                                    else { continue };

                                    if mesh.index_count == 0 {
                                        warn!("an empty mesh was generated");
                                        continue;
                                    }

                                    
                                    let normal = Direction::NORMALS[i];
                                    if dir_from_camera.dot(normal) > 0.0 {
                                        continue
                                    }

                                    let base_vertex = mesh.vertex.offset;
                                    let io = mesh.index.offset;
                                    let is = mesh.index.size;
                                    assert_eq!(is as u32, mesh.index_count as u32);
                                    pass.draw_indexed(io as u32..(io+is) as u32, base_vertex as i32, offsets.len() as u32-1..offsets.len() as u32);
                                    did_draw = true;
                                }

                                if !did_draw { offsets.pop(); }
                            }
                        }
                    }

                    if !offsets.is_empty() {
                        voxel_pipeline.model_uniform.update(&renderer.device, &renderer.queue, &offsets);

                        //event_loop.exit();
                    }

                    drop(pass);
                    println!("{:?}", game.camera);
                    renderer.staging_buffer.finish();
                    renderer.queue.submit(std::iter::once(encoder.finish()));
                    renderer.staging_buffer.recall();

                }

                output.present();

                let render_time = now.elapsed();
                println!("sim_time: {simulation_time:?} render_time: {render_time:?}");

                self.window.as_ref().unwrap().request_redraw();
            }


            WindowEvent::Resized(size) => {
                let Some(renderer) = &mut self.renderer
                else { error!("resized: no renderer found"); return; };

                renderer.config.width = size.width.min(2048);
                renderer.config.height = size.height.min(2048);
                renderer.surface.configure(&renderer.device, &renderer.config);
                renderer.voxel_pipeline.depth_buffer = DepthBuffer::new(&renderer.device, renderer.config.width, renderer.config.height);

            }
            _ => (),
        }
    }
}


fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let event_loop = EventLoop::builder().build().unwrap();

    // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
    // dispatched any events. This is ideal for games and similar applications.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        window: None,
        last_frame: Instant::now(),
        time_since_last_simulation: 0.0,
        game: Game::new(),
        renderer: None,
        input: InputManager::new(),
    };

    event_loop.run_app(&mut app).unwrap();
    return;

    /*
    let mut game = Game::new();
    let mut input = InputManager::default();

    //game.renderer.window.set_cursor_mode(glfw::CursorMode::Disabled);

    info!("loading previous save-state");
    if !fs::exists("saves/").is_ok_and(|f| f == true) {
        trace!("no previous save-state. creating files");
        let _ = fs::create_dir("saves/");
        let _ = fs::create_dir("saves/chunks/");
        game.save();
    }

    game.load();

    info!("starting game loop");
    let mut last_frame = 0.0;
    let mut time_since_last_simulation_step = 0.0;
    while !game.renderer.window.should_close() {
        let current_frame = game.renderer.glfw.get_time() as f64;

        let delta_time = (current_frame - last_frame) as f32;
        last_frame = current_frame;
        time_since_last_simulation_step += delta_time;


        // seperation for seperation sake
        process_events(&mut game.renderer, &mut input);
        game.handle_input(delta_time, &mut input);

        while time_since_last_simulation_step > game.settings.delta_tick {
            game.simulation_tick();

            time_since_last_simulation_step -= game.settings.delta_tick;
        }


        game.update_world();
        game.render(&mut input, delta_time);
    }

    game.save();*/
}



#[derive(Clone, Copy)]
pub struct PhysicsBody {
    position: DVec3,
    velocity: Vec3,

    aabb_dims: Vec3,
}


/*
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
*/

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
    inventory: [Option<Item>; 30],
    hand: usize,
    hotbar: usize,
    mining_progress: Option<u32>,
    interact_delay: f32,
    pulling: Vec<DroppedItem>,

    // this is used to rotate a structure's preview
    preview_rotation_offset: u8,

    // this is for builders ruler
    builders_ruler: Option<IVec3>,
}


impl Player {
    pub fn can_give(&self, mut item: Item) -> bool {
        for slot in &self.inventory {
            let Some(inv_item) = slot
            else { continue };

            if inv_item.kind != item.kind { continue }

            let addition = item.amount.min(inv_item.kind.max_stack_size().max(inv_item.amount) - inv_item.amount);
            item.amount -= addition;
            if item.amount == 0 {
                return true;
            }
        }


        for slot in &self.inventory {
            if slot.is_some() { continue }
            if item.amount >= item.kind.max_stack_size() {
                item.amount -= item.kind.max_stack_size();
            } else {
                item.amount = 0;
            }

            if item.amount == 0 {
                return true;
            }

        }

        false
    }


    pub fn add_item(&mut self, mut item: Item) {
        assert!(self.can_give(item));
        let (before, now) = self.inventory.split_at_mut(self.hotbar * PLAYER_HOTBAR_SIZE);
        for slot in now.iter_mut().chain(before.iter_mut()) {
            let Some(inv_item) = slot
            else { continue };

            if inv_item.kind != item.kind { continue }

            let addition = item.amount.min(inv_item.kind.max_stack_size() - inv_item.amount);
            inv_item.amount += addition;
            item.amount -= addition;
            if item.amount == 0 {
                return;
            }
        }


        for slot in now.iter_mut().chain(before.iter_mut()) {
            if slot.is_some() { continue }

            let addition = item.amount.min(item.kind.max_stack_size());

            let mut slot_item = item;
            slot_item.amount = addition;
            *slot = Some(slot_item);

            item.amount -= addition;

            if item.amount == 0 {
                return;
            }
        }
    }


    pub fn hand_index(&self) -> usize {
        self.hotbar * PLAYER_HOTBAR_SIZE + self.hand
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
        }


        Some(Item { amount, kind: slot.kind })
    }
}


#[derive(Debug)]
pub struct Camera {
    position: DVec3,
    front: Vec3,
    up: Vec3,

    pitch: f32,
    yaw: f32,

    fov: f32,
    aspect_ratio: f32,
    near: f32,
    far: f32,

}


impl Camera {
    pub fn perspective_matrix(&self) -> Mat4 {
        glam::Mat4::perspective_rh_gl(self.fov, self.aspect_ratio, self.near, self.far)
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_to_rh(Vec3::ZERO, self.front, self.up)
    }


    pub fn compass_direction(&self) -> CardinalDirection {
        let mut angle = self.yaw % TAU;

        if angle < 0.0 { angle += TAU }

        let angle = angle;
        let sector = (angle / (PI/2.0)).round() as i32 % 4;

        match sector {
            3 => CardinalDirection::South,
            0 => CardinalDirection::West,
            1 => CardinalDirection::North,
            2 => CardinalDirection::East,
            _ => unreachable!(),
        }
    }


    pub fn right(&self) -> Vec3 {
        self.up.cross(self.front)
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
        _ => (c, 0.0, x),
    };

    let m = l - c / 2.0;
    let (r, g, b) = (r1 + m, g1 + m, b1 + m);

    let to_255 = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;

    (to_255(r), to_255(g), to_255(b))
}

fn rgb_to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

// Usage
fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let (r, g, b) = hsl_to_rgb(h, s, l);
    rgb_to_hex(r, g, b)
}


