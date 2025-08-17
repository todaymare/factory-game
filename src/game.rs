pub mod save_system;

use std::{collections::HashSet, time::Instant};

use glam::{DVec3, IVec3, Mat4, Quat, Vec2, Vec3, Vec4, Vec4Swizzles};
use kira::{sound::static_sound::{StaticSoundData, StaticSoundSettings}, AudioManager, AudioManagerSettings, DefaultBackend, Tween};
use sti::hash::fxhash::fxhash32;
use tracing::{info, warn, Instrument};
use winit::{dpi::LogicalPosition, event::MouseButton, keyboard::KeyCode, window::CursorGrabMode};

use crate::{commands::{Command, CommandRegistry}, constants::{CHUNK_SIZE_I32, COLOUR_DENY, COLOUR_PASS, DELTA_TICK, DROPPED_ITEM_SCALE, LOAD_DISTANCE, MOUSE_SENSITIVITY, PLAYER_HOTBAR_SIZE, PLAYER_INTERACT_DELAY, PLAYER_INVENTORY_SIZE, PLAYER_PULL_DISTANCE, PLAYER_REACH, PLAYER_ROW_SIZE, PLAYER_SPEED, RENDER_DISTANCE, TICKS_PER_SECOND, UI_CROSSAIR_COLOUR, UI_CROSSAIR_SIZE, UI_HOTBAR_SELECTED_BG, UI_HOTBAR_UNSELECTED_BG, UI_ITEM_AMOUNT_SCALE, UI_ITEM_OFFSET, UI_ITEM_SIZE, UI_SLOT_PADDING, UI_SLOT_SIZE}, directions::CardinalDirection, entities::{EntityKind, EntityMap}, frustum::Frustum, input::InputManager, items::{Assets, Item, ItemKind, MeshIndex}, mesh::{Mesh, MeshInstance}, renderer::{Renderer, View}, structures::{strct::{Structure, StructureData, StructureKind}, Structures}, ui::{InventoryMode, UILayer, HOTBAR_KEYS}, voxel_world::{chunker::{ChunkEntry, ChunkPos, MeshEntry, WorldChunkPos}, split_world_pos, voxel::Voxel, VoxelWorld, SURROUNDING_OFFSETS}, Camera, PhysicsBody, Player, Tick};

pub struct Game {
    pub world: VoxelWorld,
    pub player: Player,
    pub entities: EntityMap,
    pub command_registry: CommandRegistry,
    pub structures: Structures,

    pub camera: Camera,
    pub current_tick: Tick,
    pub craft_queue: Vec<(Item, u32)>,
    pub craft_progress: u32,
    pub triangle_count: u32,
    pub draw_call_count: u32,
    pub render_world_time: u32,
    pub total_rendered_chunks: u32,
    pub lock_frustum: Option<Frustum>,
    pub sky_colour: Vec4,
    is_mouse_locked: bool,
    ui_layer: UILayer,

    pub settings: Settings,
    prev_player_chunk: Option<WorldChunkPos>,


    audio: AudioManager<DefaultBackend>,


}

#[derive(Clone, Copy)]
pub struct Settings {
    pub ui_scale: f32,
    pub delta_tick: f32,
    pub player_speed: f32,
    pub render_distance: i32,
    pub lines: bool,
    pub draw_hitboxes: bool,
}


impl Game {
    pub fn new() -> Game {
        let mut this = Game {
            triangle_count: 0,
            total_rendered_chunks: 0,
            draw_call_count: 0,
            render_world_time: 0,
            lock_frustum: None,
            sky_colour: Vec4::new(116.0, 217.0, 249.0, 255.0) / Vec4::splat(255.0),

            world: VoxelWorld::new(),
            structures: Structures::new(),
            entities: EntityMap::new(),

            camera: Camera {
                position: DVec3::ZERO,
                front: Vec3::Z,
                up: Vec3::new(0.0, 1.0, 0.0),
                pitch: 0.0,
                yaw: 90.0f32.to_radians(),
                fov: 80.069f32.to_radians(),
                aspect_ratio: 16.0/9.0,
                near: 0.01,
                far: 5_000.0,

            },



            player: Player {
                body: PhysicsBody {
                    position: DVec3::new(0.0, 10.0, 0.0),
                    velocity: Vec3::ZERO,
                    aabb_dims: Vec3::new(0.8, 1.8, 0.8),
                },

                inventory: [None; PLAYER_INVENTORY_SIZE],
                hand: 0,
                hotbar: PLAYER_ROW_SIZE-1,
                mining_progress: None,
                interact_delay: 0.0,
                preview_rotation_offset: 0,

            },

            current_tick: Tick::initial(),
            command_registry: CommandRegistry::new(),
            craft_queue: vec![],
            craft_progress: 0,

            ui_layer: UILayer::Gameplay { smoothed_dt: 0.0 },
            is_mouse_locked: true,

            settings: Settings {
                ui_scale: 1.0,
                delta_tick: DELTA_TICK,
                player_speed: PLAYER_SPEED,
                render_distance: RENDER_DISTANCE,
                lines: false,
                draw_hitboxes: false,
            },

            prev_player_chunk: Some(WorldChunkPos(IVec3::MAX)),


            audio: AudioManager::new(AudioManagerSettings::default()).unwrap(),
        };


        this.command_registry.register("speed", |game, cmd| {
            let speed = cmd.arg(0)?.as_f32()?;
            game.settings.player_speed = speed;
            Some(())
        });


        this.command_registry.register("rd", |game, cmd| {
            let speed = cmd.arg(0)?.as_i32()?;
            game.settings.render_distance = speed;
            game.prev_player_chunk = Some(WorldChunkPos(IVec3::MAX));
            Some(())
        });


        this.command_registry.register("unload", |game, cmd| {
            let (chunk_pos, _) = split_world_pos(game.player.body.position.as_ivec3());
            game.world.chunker.unload_voxel_data_of_chunk(chunk_pos);
            Some(())
        });


        this.command_registry.register("give", |game, cmd| {
            let item = cmd.arg(0)?.as_str();
            let &kind = ItemKind::ALL.iter().find(|x| x.to_string() == item)?;

            let amount = cmd.arg(1)?.as_u32()?;

            let stacks = amount / kind.max_stack_size();
            let rem = amount % kind.max_stack_size();
            
            for _ in 0..stacks {
                let item = Item { amount: kind.max_stack_size(), kind };
                game.entities.spawn(
                    EntityKind::dropped_item(item),
                    game.player.body.position
                );
            }

            let item = Item { amount: rem, kind };
            game.entities.spawn(
                EntityKind::dropped_item(item),
                game.player.body.position
            );

            Some(())
        });


        this.command_registry.register("tp", |game, cmd| {
            let x = cmd.arg(0)?.as_f64()?;
            let y = cmd.arg(1)?.as_f64()?;
            let z = cmd.arg(2)?.as_f64()?;
            let pos = DVec3::new(x, y, z);
            game.player.body.position = pos;

            Some(())
        });

        this.command_registry.register("clear", |game, _| {
            game.player.inventory.iter_mut().for_each(|x| *x = None);

            Some(())
        });

        this.command_registry.register("dt", |game, cmd| {
            game.settings.delta_tick = cmd.arg(0)?.as_f32()?;
            Some(())
        });

        this.command_registry.register("ui_scale", |game, cmd| {
            game.settings.ui_scale = cmd.arg(0)?.as_f32()?;
            Some(())
        });

        this.command_registry.register("toggle_frustum", |game, _| {
            if game.lock_frustum.is_some() {
                game.lock_frustum = None;
            } else {
                game.lock_frustum = Some(Frustum::compute(game.camera.perspective_matrix(), game.camera.view_matrix()));
            }
            Some(())
        });

        this
    }


    pub fn call_command(&mut self, command: Command) {
        let Some(func) = self.command_registry.find(command.command())
        else {
            self.command_registry.previous_commands.push(command);
            return;
        };

        func(self, &command);

        self.command_registry.previous_commands.push(command);
    }

    
    pub fn can_place_structure(
        &mut self,
        structure: StructureKind,
        pos: IVec3,
        direction: CardinalDirection
    ) -> bool {

        let pos = pos - structure.origin(direction);
        let blocks = structure.blocks(direction);
        for offset in blocks {
            if !self.world.get_voxel(pos + offset).is_air() {
                return false;
            }
        }

        true
    }




    pub fn handle_input(&mut self, delta_time: f32, input: &mut InputManager) {
        // handle mouse movement 
        if matches!(self.ui_layer, UILayer::Gameplay { .. }) {
            let dt = input.mouse_delta();
            if !dt.is_nan() {
                self.camera.yaw += dt.x * MOUSE_SENSITIVITY;
                self.camera.pitch -= dt.y * MOUSE_SENSITIVITY;
                
                self.camera.yaw = self.camera.yaw % 360f32.to_radians();

                self.camera.pitch = self.camera.pitch.clamp((-89.9f32).to_radians(), 89.99f32.to_radians()) % 360f32.to_radians();

                let yaw = self.camera.yaw;
                let pitch = self.camera.pitch;
                let x = yaw.cos() * pitch.cos();
                let y = pitch.sin();
                let z = yaw.sin() * pitch.cos();

                self.camera.front = Vec3::new(x, y, z).normalize();
            }


            let dt = input.scroll_delta();
            if input.is_key_pressed(KeyCode::ControlLeft) {
                if dt.y > 0.0 && self.player.hotbar == PLAYER_ROW_SIZE-1 { self.player.hotbar = 0 }
                else if dt.y > 0.0 { self.player.hotbar += 1 }
                if dt.y < 0.0 && self.player.hotbar == 0 { self.player.hotbar = PLAYER_ROW_SIZE-1 }
                else if dt.y < 0.0 { self.player.hotbar -= 1 }
            } else {
                if dt.y > 0.0 && self.player.hand == PLAYER_HOTBAR_SIZE-1 { self.player.hand = 0 }
                else if dt.y > 0.0 { self.player.hand += 1 }
                if dt.y < 0.0 && self.player.hand == 0 { self.player.hand = PLAYER_HOTBAR_SIZE-1 }
                else if dt.y < 0.0 { self.player.hand -= 1 }
            }
        }


        // handle keyboard input
        'input: {
            if input.is_key_just_pressed(KeyCode::Escape) {
                let mut ui_layer = core::mem::replace(&mut self.ui_layer, UILayer::None);
                ui_layer.close(self, delta_time);
                self.ui_layer = UILayer::Gameplay { smoothed_dt: delta_time };
            }

            if !matches!(self.ui_layer, UILayer::Gameplay { .. }) {
                break 'input;
            }


            let mut dir = Vec3::ZERO;
            if input.is_key_pressed(KeyCode::KeyW) {
                dir += self.camera.front;
            } else if input.is_key_pressed(KeyCode::KeyS) {
                dir -= self.camera.front;
            }

            if input.is_key_pressed(KeyCode::KeyD) {
                dir += self.camera.front.cross(self.camera.up);
            } else if input.is_key_pressed(KeyCode::KeyA) {
                dir -= self.camera.front.cross(self.camera.up);
            }

            if input.is_key_pressed(KeyCode::KeyC) {
                self.camera.fov = 15f32.to_radians();
            } else {
                self.camera.fov = 80f32.to_radians();
            }


            dir.y = 0.0;
            let dir = dir.normalize_or_zero();
            let mov = dir * self.settings.player_speed;
            self.player.body.velocity.x = mov.x;
            self.player.body.velocity.z = mov.z;


            if input.is_key_pressed(KeyCode::Space) {
                self.player.body.velocity.y = 5.0;
            }


            if let Some(item) = self.player.inventory[self.player.hand_index()]
                && matches!(item.kind, ItemKind::Voxel(_) | ItemKind::Structure(_)) {
                if input.is_key_just_pressed(KeyCode::KeyR) {
                    self.player.preview_rotation_offset += 1;
                    self.player.preview_rotation_offset %= 4;
                }
            } else {
                self.player.preview_rotation_offset = 0;
            }


            'i: { if input.is_key_just_pressed(KeyCode::KeyE) {
                if matches!(self.ui_layer, UILayer::Inventory { .. }) {
                    break 'i;
                } 

                let mut inv_kind = InventoryMode::Recipes;
                if let Some((raycast, _)) = self.world.raycast_voxel(self.camera.position, self.camera.front, PLAYER_REACH) {
                    let structure = self.world.structure_blocks.get(&raycast);
                    if let Some(structure) = structure {
                        let structure_kind = self.structures.get(*structure).data.as_kind();
                        if structure_kind == StructureKind::Chest {
                            inv_kind = InventoryMode::Chest(*structure);
                        } else if structure_kind == StructureKind::Silo {
                            inv_kind = InventoryMode::Silo(*structure);
                        } else if structure_kind == StructureKind::Assembler {
                            inv_kind = InventoryMode::Assembler(*structure);
                        } else if structure_kind == StructureKind::Furnace {
                            inv_kind = InventoryMode::Furnace(*structure);
                        } else if structure_kind == StructureKind::SteelFurnace {
                            inv_kind = InventoryMode::Furnace(*structure);
                        } else if structure_kind == StructureKind::Inserter {
                            inv_kind = InventoryMode::Inserter(*structure);
                        }
                    }
                }


                self.ui_layer = UILayer::Inventory {
                    just_opened: true, 
                    holding_item: None,
                    inventory_mode: inv_kind
                };
            } }


            if input.is_key_just_pressed(KeyCode::KeyG) {
                info!("generating a belt graph at 'sccs.dot'");
                let belts = self.structures.belts(&self.world);
                std::fs::write("sccs.dot", belts.scc_graph()).unwrap();
            }


            if input.is_key_just_pressed(KeyCode::KeyP) {
                self.settings.lines = !self.settings.lines;
            }


            if input.is_key_just_pressed(KeyCode::Enter) {
                if !matches!(self.ui_layer, UILayer::Console { .. }) {
                    self.ui_layer = UILayer::Console {
                        text: String::new(),
                        backspace_cooldown: 1.0,
                        timer: 0.0,
                        cursor: 0,
                        just_opened: true,
                        offset: 1
                    }
                }
            }


            if input.is_key_just_pressed(KeyCode::F3) {
                self.settings.draw_hitboxes = !self.settings.draw_hitboxes;
            }


            if input.is_key_just_pressed(KeyCode::F6) {
                info!("saving self on-command");
                let time = Instant::now();
                self.save();
                info!("saved in {:?}", time.elapsed());
            }


            if input.is_key_just_pressed(KeyCode::F7) {
                info!("loading self on-command");
                let time = Instant::now();
                self.load();
                info!("loaded save in {:?}", time.elapsed());
            }




            if input.is_key_pressed(KeyCode::KeyQ) {
                let raycast = self.world.raycast_voxel(self.camera.position,
                                                  self.camera.front,
                                                  PLAYER_REACH);
                if let Some((pos, n)) = raycast {
                    let voxel = self.world.get_voxel(pos);
                    if voxel.is_structure() {
                        let structure = self.world.structure_blocks.get(&pos).unwrap();
                        let structure = self.structures.get_mut(*structure);

                        if let StructureData::Inserter { filter, .. } = &mut structure.data {
                            *filter = None; 
                        }
                        else {
                            for index in 0..structure.available_items_len() {
                                let item = structure.try_take(index, u32::MAX);
                                if let Some(item) = item {
                                    self.entities.spawn(
                                        EntityKind::dropped_item(item),
                                        pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5) + n.as_dvec3(),
                                    );
                                    break;
                                }

                            }
                        }
                    }
                }
            }




            if input.is_key_pressed(KeyCode::ControlLeft) {
                let mut offset = None;
                if input.is_key_just_pressed(KeyCode::Digit1) { offset = Some(5) }
                if input.is_key_just_pressed(KeyCode::Digit2) { offset = Some(4) }
                if input.is_key_just_pressed(KeyCode::Digit3) { offset = Some(3) }
                if input.is_key_just_pressed(KeyCode::Digit4) { offset = Some(2) }
                if input.is_key_just_pressed(KeyCode::Digit5) { offset = Some(1) }
                if input.is_key_just_pressed(KeyCode::Digit6) { offset = Some(0) }

                if let Some(offset) = offset {
                    self.player.hotbar = offset;
                }
            } else {
                for (i, &key) in HOTBAR_KEYS.iter().enumerate() {
                    if input.is_key_just_pressed(key) { self.player.hand = i }
                }
            }
        }

        
        // handle block interactions
        'outer: {
            self.player.interact_delay -= delta_time;


            if !matches!(self.ui_layer, UILayer::Gameplay { .. }) {
                break 'outer;
            }


            'input_block: {
                if !input.is_button_pressed(MouseButton::Left) {
                    self.player.mining_progress = None;
                    break 'input_block;
                }


                let Some((pos, _))= self.world.raycast_voxel(self.camera.position,
                                                             self.camera.front,
                                                             PLAYER_REACH)
                else {
                    self.player.mining_progress = None;
                    break 'input_block;
                };


                let Some(mining_progress) = self.player.mining_progress
                else {
                    self.player.mining_progress = Some(0);
                    break 'input_block;
                };


                let voxel = self.world.get_voxel(pos);
                if mining_progress < voxel.base_hardness() {
                    break 'input_block;
                }


                let item = self.world.break_block(&mut self.structures, &mut self.entities, pos);
                self.entities.spawn(
                    EntityKind::dropped_item(item),
                    pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)
                );

                self.player.mining_progress = None;
            }



            'input_block: {
                if input.is_button_just_pressed(MouseButton::Right) {
                    self.player.interact_delay = 0.0;
                }

                if self.player.interact_delay > 0.0 {
                    break 'input_block;
                }

                if !input.is_button_pressed(MouseButton::Right) {
                    break 'input_block;
                }


                let Some((pos, normal)) = self.world.raycast_voxel(self.camera.position,
                                                                   self.camera.front,
                                                                   PLAYER_REACH)
                else { break 'input_block };

                let place_position = pos + normal;

                let voxel = self.world.get_voxel(place_position);
                if !voxel.is_air() { break 'input_block }

                let Some(Some(item_in_hand)) = self.player.inventory.get(self.player.hand_index())
                else { break 'input_block };


                if let Some(voxel) = item_in_hand.kind.as_voxel() {
                    let _ = self.player.take_item(self.player.hand_index(), 1).unwrap();

                    *self.world.get_voxel_mut(place_position) = voxel;

                } else if let Some(structure_kind) = item_in_hand.kind.as_structure() {
                    let dir = self.camera.compass_direction().next_n(self.player.preview_rotation_offset);

                    if !self.can_place_structure(structure_kind, place_position, dir) {
                        break 'input_block;
                    }

                    let structure = Structure::from_kind(structure_kind, place_position, dir);
                    let _ = self.player.take_item(self.player.hand_index(), 1).unwrap();
                    let id = self.structures.add_structure(&mut self.world, structure);

                    if structure_kind == StructureKind::Assembler {
                        self.ui_layer = UILayer::inventory_view(InventoryMode::Assembler(id))
                    }
                }


                self.player.interact_delay = PLAYER_INTERACT_DELAY;

            }
        }
    }



    pub fn simulation_tick(&mut self) {
        self.current_tick = self.current_tick.inc();

        let delta_time = DELTA_TICK;

        if self.current_tick.u32() % (TICKS_PER_SECOND * 120) == 0 {
            info!("autosaving..");
            self.save();
        }


        /*
        if self.settings.render_distance < RENDER_DISTANCE
            && self.world.chunker.mesh_load_queue_len() == 0
            && self.world.chunker.chunk_load_queue_len() == 0
            && self.world.chunker.chunk_active_jobs_len() == 0
            && self.world.chunker.mesh_active_jobs_len() == 0 {

            let player_chunk = IVec3::ZERO;
            let rd = self.settings.render_distance;

            for y in -rd..=rd {
                for z in -rd..=rd {
                    for x in -rd..=rd {
                        let offset = IVec3::new(x, y, z);
                        let dist = offset.length_squared();
                        let chunk_pos = offset + player_chunk;
                        if dist < rd*rd {
                            self.world.try_get_chunk(WorldChunkPos(chunk_pos));
                            self.world.try_get_mesh(chunk_pos);
                        }
                    }
                }
            }

            self.settings.render_distance += 1;
            self.settings.render_distance = self.settings.render_distance.min(RENDER_DISTANCE);
            println!("heyo {}", self.settings.render_distance);
        }*/


        {

            let player_chunk = self.player.body.position.as_ivec3();
            let (player_chunk, _) = split_world_pos(player_chunk);
            let ld = LOAD_DISTANCE;

            if let Some(old_chunk) = self.prev_player_chunk
                && old_chunk != player_chunk {

                let mut prev_mask = HashSet::new();
                let mut curr_mask = HashSet::new();

                let rd = self.settings.render_distance;
                if old_chunk.0 != IVec3::MAX {
                    for z in -rd..rd {
                        for y in -rd..rd {
                            for x in -rd..rd {

                                let offset = IVec3::new(x, y, z);
                                if offset.length_squared() <= rd*rd {
                                    prev_mask.insert(WorldChunkPos(old_chunk.0 + offset));
                                }
                            }
                        }
                    }
                }


                let rd = self.settings.render_distance+1;
                for z in -rd..rd {
                    for y in -rd..rd {
                        for x in -rd..rd {

                            let offset = IVec3::new(x, y, z);
                            if offset.length_squared() <= rd*rd {
                                curr_mask.insert(WorldChunkPos(player_chunk.0 + offset));
                            }
                        }
                    }
                }


                /*
                prev_mask.difference(&curr_mask)
                    .for_each(|x| self.world.chunker.unload_chunk(*x));
                    */


                curr_mask.difference(&prev_mask)
                    .for_each(|x| {
                        self.world.try_get_chunk(*x);
                        self.world.try_get_mesh(x.0);
                    });

            } else {
            }

            self.prev_player_chunk = Some(player_chunk);
        }


        if self.current_tick.u32() % (TICKS_PER_SECOND * 5) == 10000 {

            let time = Instant::now();
            let (player_chunk, _) = split_world_pos(self.player.body.position.as_ivec3());
            let rd = self.settings.render_distance-1;

            let mut unloaded = 0;

            let mut unload = vec![];

            'unload:
            for (pos, chunk, mesh) in self.world.chunker.iter_chunks() {
                if self.world.chunker.is_queued_for_unloading(pos) {
                    warn!("skipping cos queued for unloading");
                    continue;
                }

                let offset = (pos.0-player_chunk.0).length_squared();
                if offset < LOAD_DISTANCE*LOAD_DISTANCE { continue }

                let chunk = match chunk {
                    ChunkEntry::Loaded(chunk) => chunk,
                    _ => {
                        continue
                    }
                };


                let rd = self.settings.render_distance;
                let full_unload = offset > rd*rd;

                if self.world.chunker.is_queued_for_meshing(pos) {
                    warn!("skipping cos queued for meshing");
                    continue
                } else if self.world.chunker.is_chunk_meshing(pos) {
                    warn!("skipping cos meshing");
                    continue
                } else {
                    // the mesh exists
                    if offset < rd*rd {
                        match mesh {
                            MeshEntry::Loaded(mesh) => {
                                if chunk.version.get() != mesh.version.get() {
                                    warn!("skipping cos version difference");
                                    // the version mismatches
                                    continue;
                                }

                            },
                            _ => (),
                        };
                    }


                }


                
                // check that any surrounding chunk isn't gonna need it soon
                for offset in SURROUNDING_OFFSETS {
                    let pos = WorldChunkPos(pos.0 + offset);
                    if self.world.chunker.is_queued_for_meshing(pos) {
                        continue 'unload;
                    }
                }


                unload.push((full_unload, pos));
                unloaded += 1;
            }

            
            for (full, pos) in unload {
                if full {
                    self.world.chunker.unload_chunk(pos);
                } else {
                    self.world.chunker.unload_voxel_data_of_chunk(pos);
                }
            }


            warn!("checking dead chunks took {:?}, unloaded: {unloaded}, render distance {}, size: {}",
                  time.elapsed(), self.settings.render_distance, self.world.chunker.iter_chunks().count());
        }

        if !self.craft_queue.is_empty() && self.player.can_give(self.craft_queue[0].0) {
            self.craft_progress += 1;
            if self.craft_progress == self.craft_queue[0].1 {
                let (result, _) = self.craft_queue.remove(0);
                if result.amount != 0 {
                    self.player.add_item(result);
                }


                if result.kind == ItemKind::Radar {
                    let source = StaticSoundData::from_media_source(std::io::Cursor::new(include_bytes!("../congratz.wav"))).unwrap();
                    let mut sound = self.audio.play(source.clone()).unwrap();
                    sound.pause(Tween::default());
                    self.ui_layer = UILayer::Credits { time: 0.0, audio: sound }
                }

                self.craft_progress = 0;
            }
        } else {
            self.craft_progress = 0;
        }

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
                let len = self.entities.entities.len();
                for i in 0..len {
                    let Some(entity) = self.entities.entities.entry_at(i)
                    else { continue };

                    let lifetime = self.current_tick - entity.spawn_tick;
                    if entity.spawn_tick == Tick::NEVER {
                        entity.spawn_tick = self.current_tick;
                        continue;
                    }


                    let EntityKind::DroppedItem { item, is_attracted } = &mut entity.kind
                    else { continue };


                    if !*is_attracted {
                        if lifetime.u32() < (0.2 * TICKS_PER_SECOND as f32) as u32 { continue }

                        let distance = entity.body.position.distance_squared(self.player.body.position);
                        if distance.abs() as f32 > PLAYER_PULL_DISTANCE*PLAYER_PULL_DISTANCE {
                            continue;
                        }

                        if !self.player.can_give(*item) { continue };
                        *is_attracted = true;
                    } else {
                        let distance = entity.body.position.distance_squared(self.player.body.position);

                        let can_give = self.player.can_give(*item);
                        if !can_give {
                            *is_attracted = false;
                            continue;
                        }

                        if distance.abs() < 0.5 {
                            let item = *item;
                            self.entities.entities.remove_entry_at(i);
                            self.player.add_item(item);

                        } else {
                            entity.body.position = entity.body.position
                                .move_towards(
                                    self.player.body.position, 
                                    10.0 * (1.0 + distance * 0.1) * delta_time as f64
                                );
                        }
                    }

                }
            }

        }

        // handle entity physics
        {

            let len = self.entities.entities.len();
            for i in 0..len {
                let Some(entity) = self.entities.entities.entry_at(i)
                else { continue };

                self.world.move_physics_body(delta_time, &mut entity.body)
            }
        }


        self.structures.process(&mut self.entities, &mut self.world);
    }



    pub fn render(&mut self, renderer: &mut Renderer, input: &mut InputManager, delta_time: f32) {

        {
            let mut view = View::default();
            view.vstack(|view| {
                view.spacer();

                view.hstack(|view| {
                    view.spacer();
                    view.text("3");
                    view.spacer();
                });

                view.spacer();
                view.hstack(|view| {
                    view.spacer();
                    view.text("4");
                    view.spacer();
                });
               
                view.spacer();
            });


            //let spacer_unit = view.spacer_weight(1);
            let max_size = renderer.window_size();
            //let mut spacer_unit = (max_size-view.calc_min_size(renderer)) / spacer_unit;
            //view.render(renderer, max_size, Vec2::ZERO, 1);
        }



        // render entities
        let len = self.entities.entities.len();
        for i in 0..len {
            let Some(entity) = self.entities.entities.entry_at(i)
            else { continue };


            if self.settings.draw_hitboxes {
                let instance = MeshInstance {
                    modulate: Vec4::ONE,
                    model: Mat4::from_scale_rotation_translation(
                        entity.body.aabb_dims, 
                        Quat::IDENTITY, 
                        (entity.body.position - self.camera.position).as_vec3()),
                };

                renderer.draw_mesh(renderer.assets.block_outline_mesh, instance);
            }


            let EntityKind::DroppedItem { item, .. } = &mut entity.kind
            else { continue };



            let pos = entity.body.position - self.camera.position;
            let lifetime = self.current_tick - entity.spawn_tick;

            let scale = Vec3::splat(DROPPED_ITEM_SCALE);

            // vary the rotation for each item randomly
            let hash = fxhash32(&entity.spawn_tick);
            let offset = (hash % 1024) as f32;

            let rot = (lifetime.u32() as f32 + offset) / TICKS_PER_SECOND as f32;

            let instance = MeshInstance {
                modulate: Vec4::ONE,
                model: Mat4::from_scale_rotation_translation(scale, Quat::from_rotation_y(rot), pos.as_vec3()),
            };

            renderer.draw_item(
                item.kind,
                instance,
            );
        }



        // render structures
        for (_, s) in self.structures.structs.iter() {
            // TODO: frustum culling for structures
            s.render(
                &self.structures,
                &self.camera,
                renderer,
            );
        }



        'block: {
            let Some((pos, norm)) =
                self.world.raycast_voxel(self.camera.position,
                                         self.camera.front,
                                         PLAYER_REACH)
            else { break 'block };

            let held_item = self.player.inventory[self.player.hand_index()];

            if let Some(held_item) = held_item
                && matches!(held_item.kind,   ItemKind::Voxel(_)
                                            | ItemKind::Structure(_)) {

                let mut scale = Vec3::ONE;

                let dir = self.camera.compass_direction()
                    .next_n(self.player.preview_rotation_offset);

                let (origin, blocks, colour, mesh) =
                match held_item.kind {
                    ItemKind::Structure(kind) => {
                        if matches!(kind,   StructureKind::Belt
                                          | StructureKind::Splitter) {
                            scale = Vec3::new(1.0, 0.8, 1.0);
                        }

                        let origin = kind.origin(dir);
                        let can_place =
                            self.can_place_structure(kind, pos+norm, dir);

                        let colour = match can_place {
                            true => COLOUR_PASS,
                            false => COLOUR_DENY,
                        };

                        let blocks = kind.blocks(dir);

                        let mesh = renderer.assets.get_item(held_item.kind);

                        (origin, blocks, colour, mesh)
                    }


                    ItemKind::Voxel(voxel) => {
                        (IVec3::ZERO, [IVec3::ZERO].as_ref(),
                        voxel.colour(), renderer.assets.cube)
                    }
                    _ => unreachable!()
                };


                let (mesh_pos, dims) = {
                    let mut min = IVec3::MAX;
                    let mut max = IVec3::MIN;
                    let mut pos_min = IVec3::MAX;
                    let mut pos_max = IVec3::MIN;

                    let zero_zero = (pos + norm) - origin;
                    let position = zero_zero;
                    for &offset in blocks {
                        min = min.min(offset);
                        max = max.max(offset);
                        pos_min = pos_min.min(position + offset);
                        pos_max = pos_max.max(position + offset);
                    }


                    let dims = (max - min).abs().as_vec3() + Vec3::ONE;
                    let mesh_pos = (pos_min + pos_max).as_dvec3() * 0.5;
                    let mesh_pos = mesh_pos + DVec3::splat(0.5) - self.camera.position;

                    (mesh_pos, dims)
                };


                let rot = dir.as_ivec3().as_vec3();
                let rot = rot.x.atan2(rot.z) + 90f32.to_radians();


                let colour = Vec4::new(colour.x, colour.y, colour.z, 0.8);


                // draw the ghost
                // we use scale here because the mesh should be scaled
                let model = Mat4::from_scale_rotation_translation(
                    scale * Vec3::splat(0.99),
                    Quat::from_rotation_y(rot),
                    mesh_pos.as_vec3()
                );

                renderer.draw_mesh(mesh, MeshInstance { modulate: colour, model });


                // draw the outline
                // we use dims here because `block_outline_mesh` is 1x1x1
                let model = Mat4::from_scale_rotation_translation(
                    dims * Vec3::splat(1.01),
                    Quat::IDENTITY,
                    mesh_pos.as_vec3()
                );

                renderer.draw_mesh(
                    renderer.assets.block_outline_mesh,
                    MeshInstance { modulate: colour, model }
                );

                break 'block;
            }


            // well i guess it's just a mid ass block

            let voxel = self.world.get_voxel(pos);
            let (mesh_pos, dims) = match voxel {
                Voxel::StructureBlock => {
                    let strct = self.world.structure_blocks[&pos];

                    let strct = self.structures.get(strct);
                    let blocks = strct.data.as_kind().blocks(strct.direction);

                    let mut min = IVec3::MAX;
                    let mut max = IVec3::MIN;
                    let mut pos_min = IVec3::MAX;
                    let mut pos_max = IVec3::MIN;

                    let position = strct.zero_zero();
                    for &offset in blocks {
                        min = min.min(offset);
                        max = max.max(offset);
                        pos_min = pos_min.min(position + offset);
                        pos_max = pos_max.max(position + offset);
                    }

                    let dims = (max - min).abs().as_vec3() + Vec3::ONE;
                    let mesh_pos = (pos_min + pos_max).as_dvec3() * 0.5;

                    (mesh_pos, dims)
                },


                _ => (pos.as_dvec3(), Vec3::ONE)
            };

            let colour =
            if let Some(mining_progress) = self.player.mining_progress {
                let target_hardness = voxel.base_hardness();
                let progress = mining_progress as f32 / target_hardness as f32;
                let eased = 1.0 - progress.powf(3.0);
                (Vec4::ONE * eased).with_w(1.0)
            } else {
                Vec4::ONE
            };


            // the scale is slightly larger than 1 to combat z-fighting
            let model = Mat4::from_scale_rotation_translation(
                dims * Vec3::splat(1.01),
                Quat::IDENTITY,
                (mesh_pos + DVec3::splat(0.5) - self.camera.position).as_vec3()
            );


            renderer.draw_mesh(
                renderer.assets.block_outline_mesh,
                MeshInstance { modulate: colour, model }
            );
        }



        renderer.ui_scale = self.settings.ui_scale;
        // render crossair & hotbar 
        {
            let window = renderer.window_size();

            // crossair
            let midpoint = window / 2.0;
            renderer.draw_rect(
                midpoint - UI_CROSSAIR_SIZE*0.5,
                Vec2::splat(UI_CROSSAIR_SIZE),
                UI_CROSSAIR_COLOUR
            );


            // hotbar
            let bottom_midpoint = Vec2::new(midpoint.x, window.y);

            let single_slot_size = UI_SLOT_SIZE + UI_SLOT_PADDING;
            let hotbar_size = Vec2::new(
                single_slot_size * PLAYER_HOTBAR_SIZE as f32,
                single_slot_size*2.0
            );

            let mut start = bottom_midpoint - hotbar_size * 0.5;
            let hotbar = self.player.inventory.iter()
                .enumerate()
                .skip(self.player.hotbar * PLAYER_HOTBAR_SIZE)
                .take(PLAYER_HOTBAR_SIZE);

            let hand = self.player.hand_index();

            for (i, slot) in hotbar {
                let colour = if i == hand { UI_HOTBAR_SELECTED_BG }
                             else { UI_HOTBAR_UNSELECTED_BG };

                renderer.draw_rect(
                    start,
                    Vec2::splat(UI_SLOT_SIZE),
                    colour
                );

                if let Some(item) = slot {
                    renderer.draw_item_icon(
                         item.kind,
                         start+UI_ITEM_OFFSET,
                         Vec2::splat(UI_ITEM_SIZE),
                         Vec4::ONE
                    );

                    if item.amount > 0 {
                        let pos = start+UI_ITEM_OFFSET;

                        renderer.draw_text(
                            format!("{}", item.amount).as_str(),
                            Vec2::new(pos.x, pos.y),
                            UI_ITEM_AMOUNT_SCALE,
                            Vec4::ONE
                        );
                    }
                }


                start.x += single_slot_size;
            }

        }



        // render current ui layer
        let mut ui_layer = core::mem::replace(&mut self.ui_layer, UILayer::None);
        ui_layer.render(self, &input, renderer, delta_time);
        if matches!(self.ui_layer, UILayer::None) {
            self.ui_layer = ui_layer;

            let cm = self.ui_layer.is_mouse_locked();
            if self.is_mouse_locked != cm {
                self.is_mouse_locked = cm;
                let window = renderer.window_size();

                let pos = LogicalPosition::new(
                    window.x as f64 / 4.0,
                    window.y as f64 / 4.0
                );

                renderer.window.set_cursor_position(pos).unwrap();
                if cm {
                    renderer.window.set_cursor_visible(false);
                    renderer.window.set_cursor_grab(CursorGrabMode::Confined) // or Locked
                        .or_else(|_| renderer.window.set_cursor_grab(CursorGrabMode::Locked))
                        .unwrap();
                } else {
                    renderer.window.set_cursor_visible(true);
                    renderer.window.set_cursor_grab(CursorGrabMode::None).unwrap();
                }

                input.move_cursor(Vec2::NAN);
                input.move_cursor(Vec2::NAN);
                input.move_cursor(Vec2::NAN);
            }
        }





        // render "interact with structure" text
        if let Some((raycast, _)) = self.world.raycast_voxel(self.camera.position,
                                                             self.camera.front,
                                                             PLAYER_REACH)
           && let Some(structure) = self.world.structure_blocks.get(&raycast) {

            match &self.structures.get(*structure).data {
                  StructureData::Chest
                | StructureData::Silo
                | StructureData::Furnace(_)
                | StructureData::Assembler { .. } => {
                    let window = renderer.window_size();
                    
                    let text = "Press E to interact";
                    let size = renderer.text_size(&text, 0.5);
                    let size = Vec2::new(
                        window.x*0.5 - size.x*0.5,
                        window.y - UI_SLOT_PADDING*2.0 - UI_SLOT_SIZE - size.y
                    );

                    renderer.draw_text(text, size, 0.5, Vec4::ONE);

                },
                _ => (),
            }
        }

/*

        // render current ui layer
        let mut ui_layer = core::mem::replace(&mut self.ui_layer, UILayer::None);
        ui_layer.render(self, &input, dt);
        if matches!(self.ui_layer, UILayer::None) {
            self.ui_layer = ui_layer;

            let current_cm = self.renderer.window.get_cursor_mode();
            let cm = self.ui_layer.capture_mode();
            if current_cm != cm {
                self.renderer.window.set_cursor_mode(cm);

                let window = self.renderer.window.get_size();
                self.renderer.window.set_cursor_pos(window.0 as f64 / 2.0,
                                               window.1 as f64 / 2.0);
                input.move_cursor(Vec2::NAN);
                input.move_cursor(Vec2::NAN);
                input.move_cursor(Vec2::NAN);
            }
        }



        // render item in hand
        unsafe { 
            gl::Clear(gl::DEPTH_BUFFER_BIT);
        }

        let item = self.player.inventory[self.player.hand_index()];
        if let Some(item) = item {
            let mut scale = Vec3::ONE;

            let hand_offset = Vec3::new(1.0, -0.5, 1.0); // right, down, forward
            if let ItemKind::Structure(structure) = item.kind {
                let blocks = structure.blocks(CardinalDirection::North);
                let mut min = IVec3::MAX;
                let mut max = IVec3::MIN;

                for &block in blocks {
                    min = min.min(block);
                    max = max.max(block);
                }

                let size = (max - min).abs() + IVec3::ONE;
                let size = size.as_vec3().max_element();
                scale /= size.abs() * 1.5;
            }


            let model = 
                Mat4::from_rotation_y(-self.camera.yaw)
                * Mat4::from_rotation_z(self.camera.pitch)
                * Mat4::from_translation(hand_offset)
                * Mat4::from_rotation_y(33f32.to_radians())
                * Mat4::from_scale(scale * 0.8);
            self.shader_mesh.set_vec4(c"modulate", Vec4::ONE);
            let mesh = self.renderer.meshes.get(item.kind);
            self.shader_mesh.set_matrix4(c"model", model);
            mesh.draw();
        }


        self.renderer.end();
*/
    }
}


fn iterate_diff<T>(
    val: &mut T,
    a_min: IVec3, a_max: IVec3,
    b_min: IVec3, b_max: IVec3,
    mut visit_old: impl FnMut(&mut T, IVec3),
    mut visit_new: impl FnMut(&mut T, IVec3),
) {
    let [ax0, ay0, az0] = a_min.to_array();
    let [ax1, ay1, az1] = a_max.to_array();
    let [bx0, by0, bz0] = b_min.to_array();
    let [bx1, by1, bz1] = b_max.to_array();

    let ix0 = ax0.max(bx0);
    let ix1 = ax1.min(bx1);
    let iy0 = ay0.max(by0);
    let iy1 = ay1.min(by1);
    let iz0 = az0.max(bz0);
    let iz1 = az1.min(bz1);

    let overlap_exists = ix0 < ix1 && iy0 < iy1 && iz0 < iz1;

    // Iterate A - intersection
    if overlap_exists {
        for x in ax0..ax1 {
            for y in ay0..ay1 {
                for z in az0..az1 {
                    if x < ix0 || x >= ix1 ||
                       y < iy0 || y >= iy1 ||
                       z < iz0 || z >= iz1 {
                        visit_old(val, IVec3::new(x, y, z));
                    }
                }
            }
        }
    } else {
        // No overlap, all of A is unique
        for x in ax0..ax1 {
            for y in ay0..ay1 {
                for z in az0..az1 {
                    visit_old(val, IVec3::new(x, y, z));
                }
            }
        }
    }

    // Same for B - intersection
    if overlap_exists {
        for x in bx0..bx1 {
            for y in by0..by1 {
                for z in bz0..bz1 {
                    if x < ix0 || x >= ix1 ||
                       y < iy0 || y >= iy1 ||
                       z < iz0 || z >= iz1 {
                        visit_new(val, IVec3::new(x, y, z));
                    }
                }
            }
        }
    } else {
        for x in bx0..bx1 {
            for y in by0..by1 {
                for z in bz0..bz1 {
                    visit_new(val, IVec3::new(x, y, z));
                }
            }
        }
    }
}

