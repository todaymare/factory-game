#![feature(portable_simd)]
#![feature(btree_cursors)]
#![feature(str_as_str)]
#![feature(path_add_extension)]
#![feature(if_let_guard)]
#![feature(generic_arg_infer)]
#![feature(iter_array_chunks)]
#![feature(seek_stream_len)]

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
pub mod free_list;
pub mod octree;
pub mod entities;

use std::{f32::consts::{PI, TAU}, ops::{self}, time::Instant};

use constants::{CHUNK_SIZE, PLAYER_HOTBAR_SIZE};
use directions::CardinalDirection;
use frustum::Frustum;
use game::Game;
use sti::define_key;
use tracing::{error, info, trace, Level};
use voxel_world::split_world_pos;
use glam::{DVec2, DVec3, IVec3, Mat4, UVec3, Vec2, Vec3, Vec4, Vec4Swizzles};
use input::InputManager;
use items::{Item};
use renderer::{create_multisampled_framebuffer, DepthBuffer, Renderer, VoxelShaderUniform};
use wgpu::{wgt::DrawIndirectArgs, TextureViewDescriptor};
use winit::{dpi::LogicalSize, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::{CursorGrabMode, Window, WindowId}};
use winit::application::ApplicationHandler;

use crate::{constants::MSAA_SAMPLE_COUNT, renderer::RenderSettings};



define_key!(EntityId(u32));



#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;


struct App {
    renderer: Option<Renderer>,
    last_frame: Instant,
    time_since_last_simulation: f32,
    game: Game,
    input: InputManager,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop.create_window(Window::default_attributes().with_inner_size(LogicalSize::new(960, 540))).unwrap();

        window.set_cursor_visible(false);
        window.set_cursor_grab(CursorGrabMode::Confined) // or Locked
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked))
            .unwrap();

        self.game.load();

        self.renderer = Some(pollster::block_on(Renderer::new(window)));
    }


    fn device_event(
            &mut self,
            _: &ActiveEventLoop,
            _: winit::event::DeviceId,
            event: winit::event::DeviceEvent,
        ) {
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                self.input.set_delta(Vec2::new(delta.0 as f32, delta.1 as f32));
            },
            _ => (),
        }
    }


    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("closing");
                self.game.save();
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
                };

            }

            WindowEvent::CursorMoved { position: pos, .. } => {
                self.input.move_cursor(DVec2::new(pos.x, pos.y).as_vec2());
            }


            WindowEvent::KeyboardInput { event, .. } => {
                match event.state {
                    winit::event::ElementState::Pressed => {
                        self.input.set_pressed_key(event.physical_key);
                        if let Some(txt) = event.text {
                            for char in txt.chars() {
                                if char.is_ascii_control() {
                                    continue;
                                }
                                self.input.new_char(char);
                            }
                        }
                    },
                    winit::event::ElementState::Released => self.input.set_unpressed_key(event.physical_key),
                };


                if self.input.is_key_pressed(winit::keyboard::KeyCode::ShiftLeft) 
                    && self.input.is_key_just_pressed(winit::keyboard::KeyCode::Escape) {
                    event_loop.exit();
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
                
                if !game.camera.front.is_normalized() { panic!("{:?}", self.game.camera.front); }

                while self.time_since_last_simulation > game.settings.delta_tick {
                    game.simulation_tick();
                    self.time_since_last_simulation -= game.settings.delta_tick;
                }

                game.world.process(&mut renderer.voxel_pipeline.chunk_offsets, &mut renderer.voxel_pipeline.instances);


                let mut encoder = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("command-encoder"),
                });


                game.world.chunker.process_mesh_jobs(
                    3,
                    &renderer.device,
                    &mut encoder,
                    &mut renderer.staging_buffer,
                    &mut renderer.voxel_pipeline.instances,
                    &mut renderer.voxel_pipeline.chunk_offsets,
                    &mut renderer.voxel_pipeline.model_uniform,
                );


                //
                // render
                //
                
                game.render(renderer, &mut self.input, dt);
                self.input.update();

                let output = renderer.surface.get_current_texture().unwrap();
                let view = output.texture.create_view(&TextureViewDescriptor::default());

                renderer.end(encoder, &mut self.game.world, &view, RenderSettings {
                    camera: &self.game.camera,
                    skybox: self.game.sky_colour,
                    render_distance: self.game.settings.render_distance as u32,
                    frustum: self.game.lock_frustum.clone(),
                    lines: self.game.settings.lines,
                });


                output.present();

                renderer.window.request_redraw();
                println!("frame");
            }


            WindowEvent::Resized(size) => {
                let Some(renderer) = &mut self.renderer
                else { error!("resized: no renderer found"); return; };

                renderer.config.width = size.width;
                renderer.config.height = size.height;
                renderer.surface.configure(&renderer.device, &renderer.config);
                renderer.framebuffer = create_multisampled_framebuffer(&renderer.device, &renderer.config);
                renderer.voxel_pipeline.depth_buffer = DepthBuffer::new(&renderer.device, renderer.config.width, renderer.config.height, MSAA_SAMPLE_COUNT);
                renderer.ui_depth_texture = DepthBuffer::new(&renderer.device, renderer.config.width, renderer.config.height, 1);

            }
            _ => (),
        }
    }
}


fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::WARN)
        .init();
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let event_loop = EventLoop::builder().build().unwrap();

    event_loop.set_control_flow(ControlFlow::Poll);

    let mut game = Game::new();
    game.load();

    info!("loading previous save-state");
    if !std::fs::exists("saves/").is_ok_and(|f| f == true) {
        trace!("no previous save-state. creating files");
        let _ = std::fs::create_dir("saves/");
        let _ = std::fs::create_dir("saves/chunks/");
        game.save();
    }

    game.load();

    let mut app = App {
        last_frame: Instant::now(),
        time_since_last_simulation: 0.0,
        game: Game::new(),
        renderer: None,
        input: InputManager::new(),
    };

    event_loop.run_app(&mut app).unwrap();
    app.game.save();
    return;

}


#[derive(Clone, Copy, Debug)]
pub struct PhysicsBody {
    position: DVec3,
    velocity: Vec3,

    aabb_dims: Vec3,
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


pub struct Player {
    body: PhysicsBody,
    inventory: [Option<Item>; 30],
    hand: usize,
    hotbar: usize,
    mining_progress: Option<u32>,
    interact_delay: f32,

    // this is used to rotate a structure's preview
    preview_rotation_offset: u8,
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
            0 => CardinalDirection::South,
            1 => CardinalDirection::West,
            2 => CardinalDirection::North,
            3 => CardinalDirection::East,
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


