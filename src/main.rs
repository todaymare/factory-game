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

use std::{f32::consts::{PI, TAU}, fs, hash::Hash, ops::{self}, time::Instant};

use commands::{Command, CommandRegistry};
use directions::CardinalDirection;
use frustum::Frustum;
use rand::{random, seq::IndexedRandom};
use tracing::{info, trace, warn, Level};
use ui::{InventoryMode, UILayer, HOTBAR_KEYS};
use voxel_world::{chunk::{MeshState, CHUNK_SIZE}, split_world_pos, voxel::Voxel, VoxelWorld};
use glam::{DVec3, IVec3, Mat4, Quat, USizeVec2, Vec2, Vec3, Vec4, Vec4Swizzles};
use glfw::{Key, MouseButton};
use input::InputManager;
use items::{DroppedItem, Item, ItemKind};
use mesh::Mesh;
use renderer::Renderer;
use shader::{Shader, ShaderProgram};
use sti::{define_key, hash::fxhash::FxHasher32};
use structures::{strct::{Structure, StructureData, StructureKind}, Structures};

define_key!(EntityId(u32));


const MOUSE_SENSITIVITY : f32 = 0.0016;

const PLAYER_REACH : f32 = 5.0;
const PLAYER_SPEED : f32 = 10.0;
const PLAYER_PULL_DISTANCE : f32 = 3.5;
const PLAYER_INTERACT_DELAY : f32 = 0.125;
const PLAYER_HOTBAR_SIZE : usize = 5;
const PLAYER_ROW_SIZE : usize = 6;
const PLAYER_INVENTORY_SIZE : usize = PLAYER_ROW_SIZE * PLAYER_HOTBAR_SIZE;

const RENDER_DISTANCE : i32 = 8;

const DROPPED_ITEM_SCALE : f32 = 0.5;

const TICKS_PER_SECOND : u32 = 60;
const DELTA_TICK : f32 = 1.0 / TICKS_PER_SECOND as f32; 


fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .init();

    let mut renderer = Renderer::new((1920/2, 1080/2));

    let mut ui_layer = UILayer::Gameplay { smoothed_dt: 0.0 };
    let mut game = Game::new();


    let mut input = InputManager::default();

    let mut block_outline_mesh = Mesh::from_vmf("assets/models/block_outline.vmf");


    let fragment = Shader::new(&fs::read("shaders/mesh.fs").unwrap(), shader::ShaderType::Fragment).unwrap();
    let vertex = Shader::new(&fs::read("shaders/mesh.vs").unwrap(), shader::ShaderType::Vertex).unwrap();
    let mesh_shader = ShaderProgram::new(fragment, vertex).unwrap();


    let fragment = Shader::new(&fs::read("shaders/voxel.fs").unwrap(), shader::ShaderType::Fragment).unwrap();
    let vertex = Shader::new(&fs::read("shaders/voxel.vs").unwrap(), shader::ShaderType::Vertex).unwrap();
    let voxel_shader = ShaderProgram::new(fragment, vertex).unwrap();


    renderer.window.set_cursor_mode(glfw::CursorMode::Disabled);


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
    let mut builders_ruler_mesh : Option<Mesh> = None;
    while !renderer.window.should_close() {
        let current_frame = renderer.glfw.get_time() as f64;

        let delta_time = (current_frame - last_frame) as f32;
        last_frame = current_frame;
        time_since_last_simulation_step += delta_time;


        // seperation for seperation sake
        process_events(&mut renderer, &mut input);


        // handle mouse movement 
        if matches!(ui_layer, UILayer::Gameplay { .. }) {
            let dt = input.mouse_delta();
            game.camera.yaw += dt.x * MOUSE_SENSITIVITY;
            game.camera.pitch -= dt.y * MOUSE_SENSITIVITY;
            
            game.camera.yaw = game.camera.yaw % 360f32.to_radians();

            game.camera.pitch = game.camera.pitch.clamp((-89.9f32).to_radians(), 89.99f32.to_radians()) % 360f32.to_radians();

            let yaw = game.camera.yaw;
            let pitch = game.camera.pitch;
            let x = yaw.cos() * pitch.cos();
            let y = pitch.sin();
            let z = yaw.sin() * pitch.cos();

            game.camera.front = Vec3::new(x, y, z).normalize();


            let dt = input.scroll_delta();
            if input.is_key_pressed(Key::LeftControl) {
                if dt.y > 0.0 && game.player.hotbar == PLAYER_ROW_SIZE-1 { game.player.hotbar = 0 }
                else if dt.y > 0.0 { game.player.hotbar += 1 }
                if dt.y < 0.0 && game.player.hotbar == 0 { game.player.hotbar = PLAYER_ROW_SIZE-1 }
                else if dt.y < 0.0 { game.player.hotbar -= 1 }
            } else {
                if dt.y > 0.0 && game.player.hand == PLAYER_HOTBAR_SIZE-1 { game.player.hand = 0 }
                else if dt.y > 0.0 { game.player.hand += 1 }
                if dt.y < 0.0 && game.player.hand == 0 { game.player.hand = PLAYER_HOTBAR_SIZE-1 }
                else if dt.y < 0.0 { game.player.hand -= 1 }
            }
        }


        // handle keyboard input
        'input: {
            if input.is_key_just_pressed(Key::Escape) {
                ui_layer.close(&mut game, delta_time);
                ui_layer = UILayer::Gameplay { smoothed_dt: delta_time };
            }

            if !matches!(ui_layer, UILayer::Gameplay { .. }) {
                break 'input;
            }

            let mut dir = Vec3::ZERO;
            if input.is_key_pressed(Key::W) {
                dir += game.camera.front;
            } else if input.is_key_pressed(Key::S) {
                dir -= game.camera.front;
            }

            if input.is_key_pressed(Key::D) {
                dir += game.camera.front.cross(game.camera.up);
            } else if input.is_key_pressed(Key::A) {
                dir -= game.camera.front.cross(game.camera.up);
            }

            if input.is_key_pressed(Key::C) {
                game.camera.fov = 15f32.to_radians();
            } else {
                game.camera.fov = 80f32.to_radians();
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
                                                  game.camera.front,
                                                  PLAYER_REACH);
                if let Some((pos, n)) = raycast {
                    let voxel = game.world.get_voxel(pos);
                    if voxel.is_structure() {
                        let structure = game.world.structure_blocks.get(&pos).unwrap();
                        let structure = game.structures.get_mut(*structure);

                        if let StructureData::Inserter { filter, .. } = &mut structure.data {
                            *filter = None; 
                        }
                        else {
                            for index in 0..structure.available_items_len() {
                                let item = structure.try_take(index, u32::MAX);
                                if let Some(item) = item {
                                    let dropped_item = DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5) + n.as_dvec3());
                                    game.world.dropped_items.push(dropped_item);
                                    break;
                                }

                            }
                        }
                    }
                }
            }


            if let Some(item) = game.player.inventory[game.player.hand_index()]
                && matches!(item.kind, ItemKind::Voxel(_) | ItemKind::Structure(_)) {
                if input.is_key_just_pressed(Key::R) {
                    game.player.preview_rotation_offset += 1;
                    game.player.preview_rotation_offset %= 4;
                }
            } else {
                game.player.preview_rotation_offset = 0;
            }

            if let Some(item) = game.player.inventory[game.player.hand_index()]
                && item.kind != ItemKind::BuildersRuler {
                game.player.builders_ruler = None;
            }



            if input.is_key_pressed(Key::X) {
                let raycast = game.world.raycast_voxel(game.camera.position,
                                                  game.camera.front,
                                                  PLAYER_REACH);
                if let Some((pos, n)) = raycast {
                    let voxel = game.world.get_voxel(pos);
                    let item = game.player.take_item(game.player.hand_index(), 1);
                    if let Some(item) = item && voxel.is_structure() {
                        let structure = game.world.structure_blocks.get(&pos).unwrap();
                        let structure = game.structures.get_mut(*structure);

                        if let StructureData::Inserter { filter, .. } = &mut structure.data {
                            *filter = Some(item.kind);
                            game.player.add_item(item);
                        }

                        else {
                            if structure.can_accept(item) {
                                structure.give_item(item);
                            } else {
                                let dropped_item = DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5) + n.as_dvec3());
                                game.world.dropped_items.push(dropped_item);
                            }
                        }
                    } else if let Some(item) = item {
                        let dropped_item = DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5) + n.as_dvec3());
                        game.world.dropped_items.push(dropped_item);
                    }
                }
            }


            'i: { if input.is_key_just_pressed(Key::E) {
                if matches!(ui_layer, UILayer::Inventory { .. }) {
                    break 'i;
                } 

                let mut inv_kind = InventoryMode::Recipes;
                if let Some((raycast, _)) = game.world.raycast_voxel(game.camera.position, game.camera.front, PLAYER_REACH) {
                    let structure = game.world.structure_blocks.get(&raycast);
                    if let Some(structure) = structure {
                        let structure_kind = game.structures.get(*structure).data.as_kind();
                        if structure_kind == StructureKind::Chest {
                            inv_kind = InventoryMode::Chest(*structure);
                        } else if structure_kind == StructureKind::Silo {
                            inv_kind = InventoryMode::Silo(*structure);
                        } else if structure_kind == StructureKind::Assembler {
                            inv_kind = InventoryMode::Assembler(*structure);
                        }
                    }
                }


                ui_layer = UILayer::Inventory { just_opened: true, holding_item: None, inventory_mode: inv_kind };
            } }


            if input.is_key_just_pressed(Key::G) {
                let belts = game.structures.belts(&game.world);
                fs::write("sccs.dot", belts.scc_graph()).unwrap();
            }


            if input.is_key_just_pressed(Key::P) {
                renderer.is_wireframe = !renderer.is_wireframe;
            }


            if input.is_key_just_pressed(Key::Enter) {
                if !matches!(ui_layer, UILayer::Console { .. }) {
                    ui_layer = UILayer::Console { text: String::new(), backspace_cooldown: 1.0, timer: 0.0, cursor: 0, just_opened: true, offset: 1 }
                }
            }


            if input.is_key_just_pressed(Key::F6) {
                info!("saving game on-command");
                let time = Instant::now();
                game.save();
                info!("saved in {:?}", time.elapsed());
            }


            if input.is_key_just_pressed(Key::F7) {
                info!("loading game on-command");
                let time = Instant::now();
                game.load();
                info!("loaded save in {:?}", time.elapsed());
            }



            if input.is_key_pressed(Key::LeftControl) {
                let mut offset = None;
                if input.is_key_just_pressed(Key::Num1) { offset = Some(0) }
                if input.is_key_just_pressed(Key::Num2) { offset = Some(1) }
                if input.is_key_just_pressed(Key::Num3) { offset = Some(2) }
                if input.is_key_just_pressed(Key::Num4) { offset = Some(3) }
                if input.is_key_just_pressed(Key::Num5) { offset = Some(4) }
                if input.is_key_just_pressed(Key::Num6) { offset = Some(5) }

                if let Some(offset) = offset {
                    game.player.hotbar = offset;
                }
            } else {
                for (i, &key) in HOTBAR_KEYS.iter().enumerate() {
                    if input.is_key_just_pressed(key) { game.player.hand = i }
                }
            }
        }


        // handle block interactions
        'outer: {
            game.player.interact_delay -= delta_time;


            if !matches!(ui_layer, UILayer::Gameplay { .. }) {
                break 'outer;
            }


            'input_block: {
                if !input.is_button_pressed(MouseButton::Button1) {
                    game.player.mining_progress = None;
                    break 'input_block;
                }


                let Some((pos, _))= game.world.raycast_voxel(game.camera.position,
                                                             game.camera.front,
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
                if mining_progress < voxel.base_hardness() {
                    break 'input_block;
                }


                let item = game.world.break_block(&mut game.structures, pos);


                let dropped_item = DroppedItem::new(item, pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5));

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
                                                                   game.camera.front,
                                                                   PLAYER_REACH)
                else { break 'input_block };

                let place_position = pos + normal;

                let voxel = game.world.get_voxel(place_position);
                if !voxel.is_air() { break 'input_block }

                let Some(Some(item_in_hand)) = game.player.inventory.get(game.player.hand_index())
                else { break 'input_block };

                if item_in_hand.kind == ItemKind::BuildersRuler {
                    if let Some(builders_ruler) = game.player.builders_ruler {
                        let pos1 = builders_ruler;
                        let pos2 = place_position;

                        let min = pos1.min(pos2);
                        let max = pos1.max(pos2);

                        for x in min.x..=max.x {
                            for y in min.y..=max.y {
                                for z in min.z..=max.z {
                                    let pos = IVec3::new(x, y, z);
                                    if game.world.get_voxel(pos) != Voxel::Air { continue }

                                    *game.world.get_voxel_mut(pos) = Voxel::Stone;

                                }
                            }
                        }

                        game.player.builders_ruler = None;

                    } else {
                        game.player.builders_ruler = Some(place_position);
                    }


                    game.player.interact_delay = PLAYER_INTERACT_DELAY * 3.0;
                    break 'input_block;
                }

                if let Some(voxel) = item_in_hand.kind.as_voxel() {
                    let _ = game.player.take_item(game.player.hand_index(), 1).unwrap();

                    *game.world.get_voxel_mut(place_position) = voxel;

                } else if let Some(structure_kind) = item_in_hand.kind.as_structure() {
                    let dir = game.camera.compass_direction().next_n(game.player.preview_rotation_offset);

                    if !game.can_place_structure(structure_kind, place_position, dir) {
                        break 'input_block;
                    }

                    let structure = Structure::from_kind(structure_kind, place_position, dir);
                    let _ = game.player.take_item(game.player.hand_index(), 1).unwrap();
                    let id = game.structures.add_structure(&mut game.world, structure);

                    if structure_kind == StructureKind::Assembler {
                        ui_layer = UILayer::inventory_view(InventoryMode::Assembler(id))
                    }
                }


                game.player.interact_delay = PLAYER_INTERACT_DELAY;

            }
        }


        // simulate!
        {
            while time_since_last_simulation_step > DELTA_TICK {
                for _ in 0..game.simulations_per_tick {
                    game.simulation_tick();
                }
                time_since_last_simulation_step -= DELTA_TICK;
            }

        }


        game.world.process();


        // render
        {
            renderer.ui_scale = game.ui_scale;
            renderer.begin(game.sky_colour);

            let projection = game.camera.perspective_matrix();
            let view = game.camera.view_matrix();

            mesh_shader.use_program();

            mesh_shader.set_matrix4(c"projection", projection);
            mesh_shader.set_matrix4(c"view", view);
            mesh_shader.set_vec4(c"modulate", Vec4::ONE);

            let mut triangles = 0;
            let mut total_rendered = 0;
            let time = Instant::now();
            // render the world
            {
                let frustum = if let Some(frustum) = game.lock_frustum.clone() { frustum }
                              else { Frustum::compute(game.camera.perspective_matrix(), game.camera.view_matrix()) };

                voxel_shader.use_program();
                voxel_shader.set_matrix4(c"projection", projection);
                voxel_shader.set_matrix4(c"view", view);
                voxel_shader.set_vec4(c"modulate", Vec4::ONE);
                voxel_shader.set_vec3(c"cameraPos", game.camera.position.as_vec3());
                voxel_shader.set_vec3(c"fog_color", game.sky_colour.xyz());
                voxel_shader.set_f32(c"fog_density", 1.0);
                voxel_shader.set_f32(c"time", renderer.glfw.get_time() as f32);
                let fog_distance = game.player.render_distance - 1;
                voxel_shader.set_f32(c"fog_start", ((fog_distance) * CHUNK_SIZE as i32) as f32 * 0.9);
                voxel_shader.set_f32(c"fog_end", (fog_distance * CHUNK_SIZE as i32) as f32);

                let (player_chunk, _) = split_world_pos(game.player.body.position.as_ivec3());
                let rd = game.player.render_distance;

                for x in -rd..rd {
                    for y in -rd..rd {
                        for z in -rd..rd {
                            let offset = IVec3::new(x, y, z);
                            if offset.length_squared() > (rd*rd) {
                                continue;
                            }


                            let pos = player_chunk + offset;
                            let min = (pos * CHUNK_SIZE as i32).as_dvec3() - game.camera.position;
                            let max = ((pos + IVec3::ONE) * CHUNK_SIZE as i32).as_dvec3() - game.camera.position;
                            if !frustum.is_box_visible(min.as_vec3(), max.as_vec3()) {
                                continue;
                            }

                            let Some(mesh) = game.world.try_get_mesh(pos)
                            else { continue };

                            if mesh.indices == 0 {
                                warn!("an empty mesh was generated");
                                continue;
                            }

                            total_rendered += 1;

                            let offset = pos * IVec3::splat(CHUNK_SIZE as i32);
                            let offset = offset.as_dvec3() - game.camera.position;
                            let model = Mat4::from_translation(offset.as_vec3());
                            voxel_shader.set_matrix4(c"model", model);

                            mesh.draw();
                            triangles += mesh.indices;
                        }
                    }
                }
            }

            game.triangle_count = triangles;
            game.render_world_time = time.elapsed().as_millis() as _;
            game.total_rendered_chunks = total_rendered;

            mesh_shader.use_program();
            // render items
            {
                for item in game.world.dropped_items.iter().chain(game.player.pulling.iter()) {
                    let position = item.body.position - game.camera.position;

                    let scale = Vec3::splat(DROPPED_ITEM_SCALE);
                    let mut hash = FxHasher32::new();
                    item.creation_tick.0.hash(&mut hash);
                    let offset = hash.hash % 1024;
                    let rot = (((game.current_tick - item.creation_tick).u32()) as f32 + offset as f32) / TICKS_PER_SECOND as f32;

                    renderer.draw_item(&mesh_shader, item.item.kind, position.as_vec3(), scale, Vec3::new(0.0, rot, 0.0));
                }

            }


            // render structures
            {
                game.structures.for_each(|structure| {
                    structure.render(&game.structures, &game.camera, &renderer, &mesh_shader);
                });
            }


            // render block outline
            'block: {
                let Some((pos, norm)) = game.world.raycast_voxel(game.camera.position,
                                                  game.camera.front,
                                                  PLAYER_REACH)
                else { break 'block };

                let held_item = game.player.inventory[game.player.hand_index()];
                if let Some(held_item) = held_item && matches!(held_item.kind, ItemKind::Voxel(_) | ItemKind::Structure(_)) {
                    let pos = pos;
                    let mut scale = Vec3::ONE;

                    let (origin, dir, blocks, colour, mesh) = 
                    if let Some(kind) = held_item.kind.as_structure() {
                        let dir = game.camera.compass_direction()
                                  .next_n(game.player.preview_rotation_offset);
                        if matches!(kind, StructureKind::Belt | StructureKind::Splitter) {
                            scale = Vec3::new(1.0, 0.8, 1.0);
                        }

                        let origin = kind.origin(dir);
                        let can_place = game.can_place_structure(kind, pos+norm, dir);

                        let colour = match can_place {
                            true => Vec3::new(0.5, 0.8, 0.5),
                            false => Vec3::new(0.8, 0.5, 0.5),
                        };

                        (origin, dir, kind.blocks(dir), colour, renderer.meshes.get(held_item.kind))
                    }
                    else if let ItemKind::Voxel(voxel) = held_item.kind {
                        (IVec3::ZERO, CardinalDirection::North, [IVec3::ZERO].as_ref(), voxel.colour().xyz(), &renderer.meshes.cube)
                    } else { unreachable!() };

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
                    let mesh_pos = mesh_pos + DVec3::splat(0.5) - game.camera.position;

                    let rot = dir.as_ivec3().as_vec3();
                    let rot = rot.x.atan2(rot.z);
                    let rot = rot + 90f32.to_radians();


                    mesh_shader.set_vec4(c"modulate", Vec4::new(colour.x, colour.y, colour.z, 0.8));

                    let model = Mat4::from_scale_rotation_translation(dims * Vec3::splat(1.01), Quat::IDENTITY, mesh_pos.as_vec3());
                    mesh_shader.set_matrix4(c"model", model);
                    block_outline_mesh.draw();

                    let model = Mat4::from_scale_rotation_translation(scale * Vec3::splat(0.99), Quat::from_rotation_y(rot), mesh_pos.as_vec3());
                    mesh_shader.set_matrix4(c"model", model);

                    mesh.draw();

                    break 'block;
                }


                if let Some(held_item) = held_item && held_item.kind == ItemKind::BuildersRuler {
                    let place_pos = pos + norm;
                    let mut mesh_pos = place_pos.as_dvec3() + DVec3::splat(0.5);
                    let mut mesh = &renderer.meshes.cube;

                    let block = game.player.builders_ruler.unwrap_or(place_pos);
                    dbg!(block);
                    let model = Mat4::from_translation(place_pos.as_vec3() + Vec3::splat(0.5) - game.camera.position.as_vec3());
                    let model = model * Mat4::from_scale(Vec3::splat(1.0));
                    mesh_shader.set_matrix4(c"model", model);
                    mesh_shader.set_vec4(c"modulate", Vec4::new(0.0, 1.0, 0.8, 0.8));
                    block_outline_mesh.draw();

                    let model = Mat4::from_translation(block.as_vec3() + Vec3::splat(0.5) - game.camera.position.as_vec3());
                    let model = model * Mat4::from_scale(Vec3::splat(1.0));
                    mesh_shader.set_matrix4(c"model", model);
                    mesh_shader.set_vec4(c"modulate", Vec4::new(1.0, 0.4, 0.8, 0.8));
                    block_outline_mesh.draw();

                    let dims = place_pos.as_vec3() - block.as_vec3();

                    {
                        let dims = dims.floor().as_ivec3();
                        let absdims = dims.abs();
                        let absdims = absdims + absdims.signum();
                        let absdims = absdims.max(IVec3::ONE);
                        dbg!(absdims);
                        let mut arr = vec![0u32; (absdims.x*absdims.y*absdims.z) as usize];
                        assert_eq!(arr.len(), (absdims.x*absdims.y*absdims.z) as usize);


                        let mins = dims.min(IVec3::ZERO);
                        let maxs = dims.max(IVec3::ZERO);
                        for x in mins.x..=maxs.x {
                            for y in mins.y..=maxs.y {
                                for z in mins.z..=maxs.z {
                                    let pos = block + IVec3::new(x, y, z);
                                    let voxel = game.world.get_voxel(pos);
                                    if voxel.is_air() {
                                        let arr_pos = IVec3::new(x, y, z) + mins.abs();
                                        arr[(arr_pos.z * absdims.y * absdims.x + arr_pos.y * absdims.x + arr_pos.x) as usize] = u32::MAX;
                                    }
                                }
                            }
                        }

                        let mut vertices = vec![];
                        let mut indices = vec![];
                        voxel_mesher::greedy_mesh(&arr, absdims.abs(), &mut vertices, &mut indices, Vec3::ONE);

                        if let Some(builders_ruler_mesh) = &mut builders_ruler_mesh {
                            builders_ruler_mesh.destroy();
                        }

                        builders_ruler_mesh = Some(Mesh::new(&vertices, &indices));
                        mesh = builders_ruler_mesh.as_ref().unwrap();
                    };


                    mesh_pos = block.min(place_pos).as_dvec3() + dims.abs().as_dvec3() * 0.5 + DVec3::splat(0.5);

                    let mesh_pos = mesh_pos - game.camera.position;
                    let model = Mat4::from_translation(mesh_pos.as_vec3());
                    let model = model;
                    mesh_shader.set_matrix4(c"model", model);
                    mesh_shader.set_vec4(c"modulate", Vec4::new(0.4, 0.4, 0.8, 0.8));
                    mesh.draw();

                    break 'block;
                }

                let voxel = game.world.get_voxel(pos);
                let target_hardness = voxel.base_hardness();
                let mut mesh_pos = pos.as_dvec3();
                let mut dims = Vec3::ONE;

                'dims: {
                    let Some(strct) = game.world.structure_blocks.get(&pos)
                    else { break 'dims };

                    let strct = game.structures.get(*strct);
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

                    dims = (max - min).abs().as_vec3() + Vec3::ONE;
                    mesh_pos = (pos_min + pos_max).as_dvec3() * 0.5;
                };

                let mesh_pos = mesh_pos + DVec3::splat(0.5) - game.camera.position;
                let model = Mat4::from_translation(mesh_pos.as_vec3());
                let model = model * Mat4::from_scale(dims * Vec3::new(1.01, 1.01, 1.01));
                mesh_shader.set_matrix4(c"model", model);

                let modulate = if let Some(mining_progress) = game.player.mining_progress {
                    let progress = mining_progress as f32 / target_hardness as f32;
                    let eased = 1.0 - progress.powf(3.0);
                    (Vec4::ONE * eased).with_w(1.0)
                } else {
                    Vec4::ONE
                };

                mesh_shader.set_vec4(c"modulate", modulate);
                block_outline_mesh.draw();
            }

            let slot_size = 64.0;
            let padding = 16.0;

            // render hotbar
            {
                let window = renderer.window_size();
                
                renderer.draw_rect(window/2.0-Vec2::splat(4.0), Vec2::splat(8.0), Vec4::ONE);

                let bottom_centre = Vec2::new(window.x * 0.5, window.y);

                let slot_amount = PLAYER_HOTBAR_SIZE;

                let mut base = bottom_centre - Vec2::new((padding + slot_size) * slot_amount as f32 * 0.5, slot_size + padding);

                for (i, slot) in game.player.inventory.iter().enumerate().skip(game.player.hotbar*PLAYER_HOTBAR_SIZE).take(PLAYER_HOTBAR_SIZE) {
                    let colour = if i == game.player.hand_index() { Vec4::new(1.0, 0.0, 0.0, 1.0) }
                                 else { (Vec4::ONE * 0.2).with_w(1.0) };

                    renderer.draw_rect(base, Vec2::splat(slot_size), colour);
                    if let Some(item) = slot {
                        renderer.draw_item_icon(item.kind, base+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
                        renderer.draw_text(format!("{}", item.amount).as_str(), base+slot_size*0.05, 0.5, Vec4::ONE);
                    }

                    base += Vec2::new(slot_size+padding, 0.0);
                }

            }


            // render interact text
            'block: {
            if let Some((raycast, _)) = game.world.raycast_voxel(game.camera.position, game.camera.front, PLAYER_REACH) {
                let Some(structure) = game.world.structure_blocks.get(&raycast)
                else { break 'block };

                match &game.structures.get(*structure).data {
                      StructureData::Chest
                    | StructureData::Silo
                    | StructureData::Assembler { .. } => {
                        let window = renderer.window_size();
                        
                        let text = "Press E to interact";
                        let size = renderer.text_size(&text, 0.5);
                        let size = Vec2::new(
                            window.x*0.5 - size.x*0.5,
                            window.y - padding*2.0 - slot_size - size.y
                        );
                        renderer.draw_text(text, size, 0.5, Vec4::ONE);

                    },
                    _ => (),
                }
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
                input.move_cursor(Vec2::NAN);
                input.move_cursor(Vec2::NAN);
                input.move_cursor(Vec2::NAN);
            }


            unsafe { 
                gl::Clear(gl::DEPTH_BUFFER_BIT);
            }


            // render item in hand
            if let Some(item) = game.player.inventory[game.player.hand_index()] {
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
                    Mat4::from_rotation_y(-game.camera.yaw)
                    * Mat4::from_rotation_z(game.camera.pitch)
                    * Mat4::from_translation(hand_offset)
                    * Mat4::from_rotation_y(33f32.to_radians())
                    * Mat4::from_scale(scale * 0.8);
                mesh_shader.set_vec4(c"modulate", Vec4::ONE);
                let mesh = renderer.meshes.get(item.kind);
                //let model = Mat4::from_translation(game.camera.front * Vec3::new(2.0, 0.0, 1.0));
                mesh_shader.set_matrix4(c"model", model);
                mesh.draw();
            }




            renderer.end();
        }
    }


    game.save();
    block_outline_mesh.destroy();
}


pub struct Game {
    world: VoxelWorld,
    camera: Camera,
    player: Player,
    current_tick: Tick,
    structures: Structures,
    command_registry: CommandRegistry,
    simulations_per_tick: usize,
    ui_scale: f32,
    craft_queue: Vec<(Item, u32)>,
    craft_progress: u32,
    triangle_count: u32,
    render_world_time: u32,
    total_rendered_chunks: u32,
    lock_frustum: Option<Frustum>,
    sky_colour: Vec4,
}


impl Game {
    pub fn new() -> Game {
        let mut this = Game {
            triangle_count: 0,
            total_rendered_chunks: 0,
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
                fov: 80.0f32.to_radians(),
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
                speed: PLAYER_SPEED,
                render_distance: RENDER_DISTANCE,
                preview_rotation_offset: 0,

                builders_ruler: None,
            },

            current_tick: Tick::initial(),
            command_registry: CommandRegistry::new(),
            simulations_per_tick: 1,
            ui_scale: 1.0,
            craft_queue: vec![],
            craft_progress: 0,
        };


        this.command_registry.register("speed", |game, cmd| {
            let speed = cmd.arg(0)?.as_f32()?;
            game.player.speed = speed;
            Some(())
        });


        this.command_registry.register("rd", |game, cmd| {
            let speed = cmd.arg(0)?.as_i32()?;
            game.player.render_distance = speed;
            Some(())
        });



        this.command_registry.register("clear_save", |game, _| {
            *game = Game::new();
            let _ = fs::remove_dir_all("saves/chunks");
            let _ = fs::create_dir("saves/chunks");
            Some(())
        });


        this.command_registry.register("give", |game, cmd| {
            let item = cmd.arg(0)?.as_str();
            let kind = *ItemKind::ALL.iter().find(|x| x.to_string() == item)?;

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

        this.command_registry.register("ups", |game, cmd| {
            game.simulations_per_tick = cmd.arg(0)?.as_u32()? as usize;
            Some(())
        });

        this.command_registry.register("ui_scale", |game, cmd| {
            game.ui_scale = cmd.arg(0)?.as_f32()?;
            Some(())
        });

        this.command_registry.register("remesh", |game, _| {

            game.world.chunks.iter_mut().filter_map(|x| x.1.as_mut()).for_each(|x| {
                x.mesh = None;
                x.mesh_state = MeshState::ShouldUpdate;
            });
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
            if !self.world.get_voxel(pos + offset).is_air() {
                return false;
            }
        }

        true
    }


    fn simulation_tick(&mut self) {
        self.current_tick = self.current_tick.inc();

        let delta_time = DELTA_TICK;

        if self.current_tick.u32() % (TICKS_PER_SECOND * 60) == 0 {
            self.save();
        }


        if self.current_tick.u32() % TICKS_PER_SECOND == 0 
            && self.world.unload_queue.is_empty() {
            let time = Instant::now();
            let (player_chunk, _) = split_world_pos(self.player.body.position.as_ivec3());
            let rd = self.player.render_distance;
            let mins = player_chunk - IVec3::splat(rd);
            let maxs = player_chunk + IVec3::splat(rd);

            for (pos, _) in &self.world.chunks {
                if pos.x < mins.x || pos.y < mins.y || pos.z < mins.z
                    || pos.x > maxs.x || pos.y > maxs.y || pos.z > maxs.z
                {
                    self.world.unload_queue.push(*pos);
                }
            }

            trace!("checking dead chunks took {:?}", time.elapsed());
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
}


#[derive(Clone, Copy)]
pub struct PhysicsBody {
    position: DVec3,
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
    inventory: [Option<Item>; 30],
    hand: usize,
    hotbar: usize,
    mining_progress: Option<u32>,
    interact_delay: f32,
    pulling: Vec<DroppedItem>,
    speed: f32,
    render_distance: i32,
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


