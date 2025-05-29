#![feature(duration_millis_float)]
#![feature(portable_simd)]
#![feature(btree_cursors)]

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

use core::{f32, time};
use std::{char, collections::{HashMap, HashSet}, f32::consts::{PI, TAU}, fmt::{Display, Write}, fs, io::BufReader, ops::{self, Bound}, simd::f32x4, time::Instant};

use directions::CardinalDirection;
use voxel_world::{chunk::{Chunk, CHUNK_SIZE}, split_world_pos, voxel::{Voxel, VoxelKind}, VoxelWorld};
use glam::{IVec3, Mat4, Vec2, Vec3, Vec4};
use glfw::{GlfwReceiver, Key, MouseButton, WindowEvent};
use input::InputManager;
use items::{DroppedItem, Item, ItemKind, ItemMeshes};
use mesh::{Mesh, Vertex};
use rand::{random, seq::IndexedRandom};
use renderer::Renderer;
use shader::{Shader, ShaderProgram};
use sti::{define_key, key::Key as _, println, vec::KVec};
use structures::{belts::SccId, strct::{rotate_block_vector, Structure, StructureKind}, StructureId, Structures};

define_key!(EntityId(u32));


const MOUSE_SENSITIVITY : f32 = 0.1;

const PLAYER_REACH : f32 = 5.0;
const PLAYER_SPEED : f32 = 5.0;
const PLAYER_INTERACT_DELAY : f32 = 0.2;

const RENDER_DISTANCE : i32 = 1;

const DROPPED_ITEM_SCALE : f32 = 0.25;


fn main() {
    let mut game = Game {
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

            inventory: Vec::new(),
            hand: 0,
            mining_progress: None,
            interact_delay: 0.0,
            pulling: Vec::new(),
        },

        current_tick: Tick::initial(),
    };


    for kind in ItemKind::ALL.iter().copied() {
        game.player.add_item(Item { amount: 99, kind });

    }

    let mut input = InputManager::default();

    for x in -RENDER_DISTANCE..RENDER_DISTANCE {
        for y in -RENDER_DISTANCE..RENDER_DISTANCE {
            for z in -RENDER_DISTANCE..RENDER_DISTANCE {
                let _= game.world.get_chunk(IVec3::new(x, y, z));
            }
        }
    }


    let mut renderer = Renderer::new((1920/2, 1080/2));

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
        }


        // handle keyboard input
        {
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
            let mov = dir * PLAYER_SPEED;
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

                        if let Some(item) = item {
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

            if input.is_key_just_pressed(Key::G) {
                let belts = game.structures.belts(&game.world);

                // export nodes graph
                let mut output = String::new();
                let _ = write!(output, "digraph {{");
                let _ = write!(output, "node [shape=box];");
                let _ = write!(output, "edge [color=gray];");


                let step = 360.0 / belts.scc_ends.len() as f64;
                for i in belts.scc_ends.krange() {
                    let hue = step * i.usize() as f64;

                    let hex = hsl_to_hex(hue, 0.6, 0.8);


                    let _ = write!(output, "subgraph cluster_{} {{", i.usize());
                    let _ = write!(output, "label = \"SCC #{} is_edge: {}\";", i.usize(), belts.edges.contains(&i));
                    let _ = write!(output, "style = filled;");
                    let _ = write!(output, "fillcolor = \"{hex}\";");

                    let scc_begin = if i == SccId::ZERO { SccId::ZERO }
                                    else { belts.scc_ends[unsafe { SccId::from_usize_unck(i.usize() - 1) }] };
                    let scc_end = belts.scc_ends[i];
                    let scc_node_ids = &belts.scc_data[scc_begin..scc_end];

                    for &scc_node_id in scc_node_ids {
                        let node = belts.nodes[scc_node_id].as_ref().unwrap();
                        let scc_node = &belts.scc_nodes[scc_node_id];

                        let _ = write!(output, "{} [label=\"node_id={} index={} lowest_link={}\"];", scc_node_id.usize(), scc_node_id.usize(), scc_node.index, scc_node.low_link);
                        for link in &node.outputs {
                            if let Some(link) = link {
                                let _ = write!(output, "{} -> {};", scc_node_id.usize(), link.usize());
                            }
                        }
                    }

                    let _ = write!(output, "}}");

                }
                let _ = write!(output, "}}");
                fs::write("sscs.dot", output.as_bytes()).unwrap();


                let mut output = String::new();

                let _ = write!(output, "digraph {{");
                let _ = write!(output, "node [shape=box];");
                let _ = write!(output, "edge [color=gray];");
                for (structure, id) in belts.structure_to_node {
                    let structure = game.structures.structs.get(structure.0).unwrap();
                    let _ = write!(output, "{} [label=\"position: {}, direction: {:?}\"];", id.usize(), structure.position, structure.direction);

                    let node = belts.nodes[id].as_ref().unwrap();
                    if let Some(out) = node.outputs[0] {
                        let _ = write!(output, "{} -> {};", id.usize(), out.usize());
                    }
                }

                let _ = write!(output, "}}");

                fs::write("nodes.dot", output.as_bytes()).unwrap();
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

                let Some(item_in_hand) = game.player.inventory.get(game.player.hand)
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


            // render debug text
            {
                let mut text = String::new();

                let fps = (1.0 / delta_time).round();
                let colour_code = if fps > 55.0 { 'a' } else if fps > 25.0 { '6' } else { '4' };

                let _ = writeln!(text, "§eFPS: §{colour_code}{fps}§r");

                let _ = writeln!(text, "§ePITCH: §a{:.1} §eYAW: §a{:.1}§r", game.camera.pitch.to_degrees(), game.camera.yaw.to_degrees());
                let _ = writeln!(text, "§ePOSITION: §a{:.1}, {:.1} {:.1}§r", game.camera.position.x, game.camera.position.y, game.camera.position.z);

                let (chunk_pos, chunk_local_pos) = split_world_pos(game.player.body.position.floor().as_ivec3());
                let _ = writeln!(text, "§eCHUNK POSITION: §a{}, {}, {}§r", chunk_pos.x, chunk_pos.y, chunk_pos.z);
                let _ = writeln!(text, "§eCHUNK LOCAL POSITION: §a{}, {}, {}§r", chunk_local_pos.x, chunk_local_pos.y, chunk_local_pos.z);
                let _ = writeln!(text, "§eCHUNK COUNT: §a{}§r", game.world.chunks.len());
                let _ = writeln!(text, "§eDIRECTION: §b{:?}§r", game.camera.compass_direction());
                let _ = writeln!(text, "§eDIRECTION VECTOR: §b{:?}§r", game.camera.compass_direction().as_ivec3());


                let target_block = game.world.raycast_voxel(game.camera.position, game.camera.direction, PLAYER_REACH);
                if let Some(target_block) = target_block {
                    let target_voxel = game.world.get_voxel(target_block.0);
                    let target_voxel_kind = target_voxel.kind;


                    let _ = writeln!(text, "§eTARGET LOCATION: §a{}, {}, {}", target_block.0.x, target_block.0.y, target_block.0.z);


                    let _ = write!(text, "§eTARGET BLOCK: §b");


                    if target_voxel.kind.is_structure() {
                       let structure = game.world.structure_blocks.get(&target_block.0).unwrap();
                       let structure = game.structures.get(*structure);

                       let _ = writeln!(text, "Structure");
                       let _ = writeln!(text, "§e- POSITION: §a{}, {}, {}", structure.position.x, structure.position.y, structure.position.z);
                       let _ = writeln!(text, "§e- DIRECTION: §b{:?}", structure.direction);
                       let _ = writeln!(text, "§e- IS ASLEEP: §b{}", structure.is_asleep);

                       let _ = write!(text, "§e- KIND: §b");

                       match &structure.data {
                            structures::strct::StructureData::Quarry { current_progress, output } => {
                                let _ = writeln!(text, "Quarry:");
                                let _ = writeln!(text, "§e    - CURRENT PROGRESS: §a{}", current_progress);
                                if let Some(output) = output {
                                    let _ = writeln!(text, "§e  - OUTPUT: §b{:?}", output);
                                } else {
                                    let _ = writeln!(text, "§e  - OUTPUT: §bEmpty");
                                }
                            },
                            structures::strct::StructureData::Inserter { state } => {
                                let _ = writeln!(text, "Inserter:");
                                match state {
                                    structures::strct::InserterState::Searching => {
                                        let _ = writeln!(text, "§e  - STATE: §aSearching");
                                    },
                                    structures::strct::InserterState::Placing(item) => {
                                        let _ = writeln!(text, "§e  - STATE: §bPlacing");
                                        let _ = writeln!(text, "§e    - ITEM: §b{:?}", item);
                                    },
                                }
                            },


                            structures::strct::StructureData::Chest { inventory } => {
                                let _ = writeln!(text, "Chest");
                                let _ = writeln!(text, "§e    - INPUT:");

                                for slot in inventory {
                                    if let Some(item) = slot.item {
                                        let _ = writeln!(text, "§e      - §b{:?} §e{}x/{}x", item.kind, item.amount, slot.max);
                                    } else if let Some(exp) = slot.expected {
                                        let _ = writeln!(text, "§e      - §b{:?} §e0/{}", exp, slot.max);
                                    } else {
                                        let _ = writeln!(text, "§e      - §bEmpty §e0/{}", slot.max);
                                    }
                               }
                            }


                            structures::strct::StructureData::Belt { inventory } => {
                                let _ = writeln!(text, "Belt");
                                let _ = writeln!(text, "§e  - INVENTORY:");
                                let _ = writeln!(text, "§e    - LEFT LANE:");
                                for item in inventory[0] {
                                    if let Some(item) = item {
                                        let _ = writeln!(text, "§e      - §b{item:?}");
                                    } else {
                                        let _ = writeln!(text, "§e      - §bEmpty");
                                    }
                                }

                                let _ = writeln!(text, "§e    - RIGHT LANE:");
                                for item in inventory[1] {
                                    if let Some(item) = item {
                                        let _ = writeln!(text, "§e      - §b{item:?}");
                                    } else {
                                        let _ = writeln!(text, "§e      - §bEmpty");
                                    }
                                }
                            }
                        }

                       /*
                       if let Some(input) = &structure.input {
                           let _ = writeln!(text, "§e- INPUT:");

                           for slot in input {
                               if let Some(item) = slot.item {
                                   let _ = writeln!(text, "§e  - §b{:?} §e{}x/{}x", item.kind, item.amount, slot.max);
                               } else if let Some(exp) = slot.expected {
                                   let _ = writeln!(text, "§e  - §b{:?} §e0/{}", exp, slot.max);
                               } else {
                                   let _ = writeln!(text, "§e  - §bEmpty §e0/{}", slot.max);
                               }
                           }
                       }

                       if let Some(slot) = &structure.output{
                           let _ = writeln!(text, "§e- OUTPUT:");

                           if let Some(item) = slot.item {
                               let _ = writeln!(text, "§e  - §b{:?} §e{}x/{}x", item.kind, item.amount, slot.max);
                           } else if let Some(exp) = slot.expected {
                               let _ = writeln!(text, "§e  - §b{:?} §e0x/{}", exp, slot.max);
                           } else {
                               let _ = writeln!(text, "§e  - §bEmpty §e0x/{}x", slot.max);
                           }
                       }*/
                    } else {

                       let _ = writeln!(text, "{:?}", target_voxel.kind);
                    }


                    if let Some(mining_progress) = game.player.mining_progress {
                        let _ = writeln!(text, "§eMINING PROGRESS: §a{}/{}",
                                         mining_progress, target_voxel_kind.base_hardness());
                    }
                }


                if !game.structures.work_queue.entries.is_empty() {
                    let mut cursor = game.structures.work_queue.entries.lower_bound(Bound::Unbounded);
                    let _ = writeln!(text, "§eWORK QUEUE:");

                    let mut i = 0;
                    while let Some(((tick, id), ())) = cursor.next() {
                        let _ = writeln!(text, "§e- §b{:?}§e in §a{} §eticks", game.structures.get(*id).data.as_kind(), (*tick - game.current_tick).u32());
                        i += 1;
                        if i > 3 {
                            let len = game.structures.work_queue.entries.len();
                            let rem = len - i;

                            if rem == 1 {
                                let _ = writeln!(text, "§7   ..1 more item");
                            } else if rem > 1 {
                                let _ = writeln!(text, "§7   ..{} more items", len - i);
                            }

                            break;
                        }
                    }
                }
                


                if !game.world.dropped_items.is_empty() {
                    let _ = writeln!(text, "§eDROPPED ITEMS:");


                    for dropped_item in game.world.dropped_items.iter() {
                        let _ = writeln!(text, "§e- §b{:?}§e: §a{:.1}, {:.1}, {:.1}", dropped_item.item, dropped_item.body.position.x, dropped_item.body.position.y, dropped_item.body.position.z);
                    }

                }


                renderer.draw_text(&text, Vec2::ZERO, 0.4, Vec3::ONE);
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
    world: VoxelWorld,
    camera: Camera,
    player: Player,
    current_tick: Tick,
    structures: Structures,
}


const TICKS_PER_SECOND : u32 = 60;
const DELTA_TICK : f32 = 1.0 / TICKS_PER_SECOND as f32; 


impl Game {
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
                    println!("{:?} {:?} {lifetime:?}", self.current_tick, item.creation_tick);
                    if lifetime.u32() < (0.2 * TICKS_PER_SECOND as f32) as u32 { i += 1; continue }

                    let distance = item.body.position.distance_squared(self.player.body.position);
                    if distance.abs() > 25.0 {
                        i += 1;
                        continue;
                    }

                    let item = self.world.dropped_items.remove(i);
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


