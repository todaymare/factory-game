pub mod save_system;

use std::time::Instant;

use glam::{DVec3, IVec3, Vec3, Vec4};
use tracing::{info, warn};
use winit::{event::MouseButton, keyboard::KeyCode};

use crate::{commands::{Command, CommandRegistry}, directions::CardinalDirection, frustum::Frustum, input::InputManager, items::{DroppedItem, Item, ItemKind}, structures::{strct::{Structure, StructureData, StructureKind}, Structures}, ui::{InventoryMode, UILayer, HOTBAR_KEYS}, voxel_world::{chunker::{ChunkEntry, MeshEntry, WorldChunkPos}, split_world_pos, voxel::Voxel, VoxelWorld, SURROUNDING_OFFSETS}, Camera, PhysicsBody, Player, Tick, constants::{DELTA_TICK, MOUSE_SENSITIVITY, PLAYER_HOTBAR_SIZE, PLAYER_INTERACT_DELAY, PLAYER_INVENTORY_SIZE, PLAYER_PULL_DISTANCE, PLAYER_REACH, PLAYER_ROW_SIZE, PLAYER_SPEED, RENDER_DISTANCE, TICKS_PER_SECOND}};

pub struct Game {
    pub world: VoxelWorld,
    pub player: Player,
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
    ui_layer: UILayer,

    pub settings: Settings,

    //block_outline_mesh: Mesh,

    //shader_mesh: ShaderProgram,
    //shader_world: ShaderProgram,
}

#[derive(Clone, Copy)]
pub struct Settings {
    pub ui_scale: f32,
    pub delta_tick: f32,
    pub player_speed: f32,
    pub render_distance: i32,
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

            camera: Camera {
                position: DVec3::ZERO,
                front: Vec3::Z,
                up: Vec3::new(0.0, 1.0, 0.0),
                pitch: 0.0,
                yaw: 90.0f32.to_radians(),
                fov: 80.069f32.to_radians(),
                aspect_ratio: 16.0/9.0,
                near: 0.05,
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
                pulling: Vec::new(),
                preview_rotation_offset: 0,

                builders_ruler: None,
            },

            current_tick: Tick::initial(),
            command_registry: CommandRegistry::new(),
            craft_queue: vec![],
            craft_progress: 0,

            ui_layer: UILayer::Gameplay { smoothed_dt: 0.0 },
            settings: Settings {
                ui_scale: 1.0,
                delta_tick: DELTA_TICK,
                player_speed: PLAYER_SPEED,
                render_distance: 1,
            },

        };


        this.command_registry.register("speed", |game, cmd| {
            let speed = cmd.arg(0)?.as_f32()?;
            game.settings.player_speed = speed;
            Some(())
        });


        this.command_registry.register("rd", |game, cmd| {
            let speed = cmd.arg(0)?.as_i32()?;
            game.settings.render_distance = speed;
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
                game.world.dropped_items.push(DroppedItem::new(item, game.player.body.position));
            }

            let item = Item { amount: rem, kind };
            game.world.dropped_items.push(DroppedItem::new(item, game.player.body.position));

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

    
    pub fn can_place_structure(&mut self,
                           structure: StructureKind,
                           pos: IVec3,
                           direction: CardinalDirection) -> bool {

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

            if input.is_key_pressed(KeyCode::KeyJ) {
                self.player.body.position.y += 5.0;
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
                                    let dropped_item = DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5) + n.as_dvec3());
                                    self.world.dropped_items.push(dropped_item);
                                    break;
                                }

                            }
                        }
                    }
                }
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

            if let Some(item) = self.player.inventory[self.player.hand_index()]
                && item.kind != ItemKind::BuildersRuler {
                self.player.builders_ruler = None;
            }



            if input.is_key_pressed(KeyCode::KeyX) {
                let raycast = self.world.raycast_voxel(self.camera.position,
                                                  self.camera.front,
                                                  PLAYER_REACH);
                if let Some((pos, n)) = raycast {
                    let voxel = self.world.get_voxel(pos);
                    let item = self.player.take_item(self.player.hand_index(), 1);
                    if let Some(item) = item && voxel.is_structure() {
                        let structure = self.world.structure_blocks.get(&pos).unwrap();
                        let structure = self.structures.get_mut(*structure);

                        if let StructureData::Inserter { filter, .. } = &mut structure.data {
                            *filter = Some(item.kind);
                            self.player.add_item(item);
                        }

                        else {
                            if structure.can_accept(item) {
                                structure.give_item(item);
                            } else {
                                let dropped_item = DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5) + n.as_dvec3());
                                self.world.dropped_items.push(dropped_item);
                            }
                        }
                    } else if let Some(item) = item {
                        let dropped_item = DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5) + n.as_dvec3());
                        self.world.dropped_items.push(dropped_item);
                    }
                }
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


            if input.is_key_pressed(KeyCode::ControlLeft) {
                let mut offset = None;
                if input.is_key_just_pressed(KeyCode::Numpad1) { offset = Some(0) }
                if input.is_key_just_pressed(KeyCode::Numpad2) { offset = Some(1) }
                if input.is_key_just_pressed(KeyCode::Numpad3) { offset = Some(2) }
                if input.is_key_just_pressed(KeyCode::Numpad4) { offset = Some(3) }
                if input.is_key_just_pressed(KeyCode::Numpad5) { offset = Some(4) }
                if input.is_key_just_pressed(KeyCode::Numpad6) { offset = Some(5) }

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


                let item = self.world.break_block(&mut self.structures, pos);


                let dropped_item = DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5));

                self.world.dropped_items.push(dropped_item);
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

                if item_in_hand.kind == ItemKind::BuildersRuler {
                    if let Some(builders_ruler) = self.player.builders_ruler {
                        let pos1 = builders_ruler;
                        let pos2 = place_position;

                        let min = pos1.min(pos2);
                        let max = pos1.max(pos2);

                        for x in min.x..=max.x {
                            for y in min.y..=max.y {
                                for z in min.z..=max.z {
                                    let pos = IVec3::new(x, y, z);
                                    if self.world.get_voxel(pos) != Voxel::Air { continue }

                                    *self.world.get_voxel_mut(pos) = Voxel::Stone;

                                }
                            }
                        }

                        self.player.builders_ruler = None;

                    } else {
                        self.player.builders_ruler = Some(place_position);
                    }


                    self.player.interact_delay = PLAYER_INTERACT_DELAY * 3.0;
                    break 'input_block;
                }

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
            //self.save();
        }


        if self.settings.render_distance < RENDER_DISTANCE
            && self.world.chunker.mesh_load_queue_len() == 0
            && self.world.chunker.chunk_load_queue_len() == 0
            && self.world.chunker.chunk_active_jobs_len() == 0
            && self.world.chunker.mesh_active_jobs_len() == 0 {

            let player_chunk = IVec3::ZERO;
            let rd = self.settings.render_distance;
            let rdp1 = rd+1;


            for y in -rd..=rd {
                for z in -rd..=rd {
                    for x in -rd..=rd {
                        let offset = IVec3::new(x, y, z);
                        let dist = offset.length_squared();
                        let chunk_pos = offset + player_chunk;

                        let entry = self.world.chunker.get_mesh_entry(WorldChunkPos(chunk_pos));
                        if !matches!(entry, MeshEntry::None) {
                            continue;
                        }

                        if dist < rdp1*rdp1 {
                            self.world.try_get_chunk(chunk_pos);
                        }

                        if dist < rd*rd {
                            self.world.try_get_mesh(chunk_pos);
                        }
                    }
                }
            }

            self.settings.render_distance += 1;
            self.settings.render_distance = self.settings.render_distance.min(RENDER_DISTANCE);
        }


        if self.current_tick.u32() % (TICKS_PER_SECOND * 5) == 0 {

            let time = Instant::now();
            let (player_chunk, _) = split_world_pos(self.player.body.position.as_ivec3());
            let rd = self.settings.render_distance-1;

            let mut skipped = 0;
            let mut unloaded = 0;

            let mut unload = vec![];

            'unload:
            for (pos, chunk, mesh) in self.world.chunker.iter_chunks() {
                let offset = (pos.0-player_chunk).length_squared();

                let chunk = match chunk {
                    ChunkEntry::Loaded(chunk) => chunk,
                    _ => continue
                };

                let mesh = match mesh {
                    MeshEntry::Loaded(mesh) => mesh,
                    _ => continue
                };

                if self.world.chunker.is_queued_for_meshing(pos) {
                    continue
                }

                if self.world.chunker.is_chunk_meshing(pos) {
                    continue
                }

                if self.world.chunker.is_queued_for_unloading(pos) {
                    continue;
                }

                
                // the mesh exists
                if offset < rd*rd {
                    if chunk.version != mesh.version {
                        // the version mismatches
                        skipped += 1;
                        continue;
                    }
                }

                // check that any surrounding chunk isn't gonna need it soon
                for offset in SURROUNDING_OFFSETS {
                    let pos = WorldChunkPos(pos.0 + offset);
                    if self.world.chunker.is_queued_for_meshing(pos) {
                        continue 'unload;
                    }
                }


                let full_unload = offset > RENDER_DISTANCE*RENDER_DISTANCE;

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


            warn!("checking dead chunks took {:?}, skipped {skipped}, unloaded: {unloaded}, render distance {}, size: {}",
                  time.elapsed(), self.settings.render_distance, self.world.chunker.iter_chunks().count());
        }

        if !self.craft_queue.is_empty() && self.player.can_give(self.craft_queue[0].0) {
            self.craft_progress += 1;
            if self.craft_progress == self.craft_queue[0].1 {
                let (result, _) = self.craft_queue.remove(0);
                if result.amount != 0 {
                    self.player.add_item(result);
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
                    if distance.abs() as f32 > PLAYER_PULL_DISTANCE*PLAYER_PULL_DISTANCE {
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
                        item.body.position = item.body.position.move_towards(self.player.body.position, 10.0 * (1.0 + distance * 0.1) * delta_time as f64);
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

    /*
        // render meshes
        self.shader_mesh.use_program();
        self.shader_mesh.set_matrix4(c"projection", projection);
        self.shader_mesh.set_matrix4(c"view", view);
        self.shader_mesh.set_vec4(c"modulate", Vec4::ONE);

        // render dropped items
        let items = self.world.dropped_items.iter()
            .chain(self.player.pulling.iter());

        for dropped_item in items {
            let pos = dropped_item.body.position - camera;
            let lifetime = self.current_tick - dropped_item.creation_tick;

            let scale = Vec3::splat(DROPPED_ITEM_SCALE);

            // vary the rotation for each item randomly
            let hash = fxhash32(&dropped_item.creation_tick);
            let offset = (hash % 1024) as f32;

            let rot = (lifetime.u32() as f32 + offset) / TICKS_PER_SECOND as f32;

            self.renderer.draw_item(
                &self.shader_mesh,
                dropped_item.item.kind,
                pos.as_vec3(),
                scale,
                Vec3::new(0.0, rot, 0.0),
            );
        }


        // render structures
        for (_, s) in self.structures.structs.iter() {
            // TODO: frustum culling for structures
            s.render(
                &self.structures,
                &self.camera,
                &self.renderer,
                &self.shader_mesh,
            );
        }


        // render block outline
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

                        let mesh = self.renderer.meshes.get(held_item.kind);

                        (origin, blocks, colour, mesh)
                    }


                    ItemKind::Voxel(voxel) => {
                        (IVec3::ZERO, [IVec3::ZERO].as_ref(),
                        voxel.colour().xyz(), &self.renderer.meshes.cube)
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
                    let mesh_pos = mesh_pos + DVec3::splat(0.5) - camera;

                    (mesh_pos, dims)
                };


                let rot = dir.as_ivec3().as_vec3();
                let rot = rot.x.atan2(rot.z) + 90f32.to_radians();


                let colour = Vec4::new(colour.x, colour.y, colour.z, 0.8);
                self.shader_mesh.set_vec4(c"modulate", colour);


                // draw the ghost
                // we use scale here because the mesh should be scaled
                let model = Mat4::from_scale_rotation_translation(
                    scale * Vec3::splat(0.99),
                    Quat::from_rotation_y(rot),
                    mesh_pos.as_vec3()
                );

                self.shader_mesh.set_matrix4(c"model", model);
                mesh.draw();


                // draw the outline
                // we use dims here because `block_outline_mesh` is 1x1x1
                let model = Mat4::from_scale_rotation_translation(
                    dims * Vec3::splat(1.01),
                    Quat::IDENTITY,
                    mesh_pos.as_vec3()
                );

                self.shader_mesh.set_matrix4(c"model", model);
                self.block_outline_mesh.draw();

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
                (mesh_pos + DVec3::splat(0.5) - camera).as_vec3()
            );

            self.shader_mesh.set_matrix4(c"model", model);
            self.shader_mesh.set_vec4(c"modulate", colour);
            self.block_outline_mesh.draw();
        }


        // render crossair & hotbar 
        {
            let window = self.renderer.window_size();

            // crossair
            let midpoint = window / 2.0;
            self.renderer.draw_rect(
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

                self.renderer.draw_rect(
                    start,
                    Vec2::splat(UI_SLOT_SIZE),
                    colour
                );

                if let Some(item) = slot {
                    self.renderer.draw_item_icon(
                         item.kind,
                         start+UI_ITEM_OFFSET,
                         Vec2::splat(UI_ITEM_SIZE),
                         Vec4::ONE
                     );

                    if item.amount > 0 {
                        self.renderer.draw_text(
                            format!("{}", item.amount).as_str(),
                            start+UI_ITEM_OFFSET,
                            UI_ITEM_AMOUNT_SCALE,
                            Vec4::ONE
                        );
                    }
                }


                start.x += single_slot_size;
            }

        }



        // render "interact with structure" text
        if let Some((raycast, _)) = self.world.raycast_voxel(camera,
                                                             self.camera.front,
                                                             PLAYER_REACH)
           && let Some(structure) = self.world.structure_blocks.get(&raycast) {

            match &self.structures.get(*structure).data {
                  StructureData::Chest
                | StructureData::Silo
                | StructureData::Assembler { .. } => {
                    let window = self.renderer.window_size();
                    
                    let text = "Press E to interact";
                    let size = self.renderer.text_size(&text, 0.5);
                    let size = Vec2::new(
                        window.x*0.5 - size.x*0.5,
                        window.y - UI_SLOT_PADDING*2.0 - UI_SLOT_SIZE - size.y
                    );

                    self.renderer.draw_text(text, size, 0.5, Vec4::ONE);

                },
                _ => (),
            }
        }


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
    }*/
}



impl Drop for Game {
    fn drop(&mut self) {
        //self.block_outline_mesh.destroy();
    }
}
