use std::{fmt::Write, ops::Bound};

use glam::{Vec2, Vec3, Vec4};
use glfw::{get_key_name, get_key_scancode, CursorMode, Key};

use crate::{commands::Command, input::InputManager, renderer::Renderer, structures::strct::{InserterState, StructureData}, voxel_world::split_world_pos, Game, PLAYER_REACH};

pub enum UILayer {
    Inventory {
        just_opened: bool,
    },
    Console {
        text: String,
        backspace_cooldown: f32,
        timer: f32,
        cursor: u32,
        just_opened: bool,
        offset: u32,
    },
    Gameplay,
}


impl UILayer {
    pub fn capture_mode(&self) -> CursorMode {
        match self {
            UILayer::Gameplay => CursorMode::Disabled,
            UILayer::Inventory { .. } => CursorMode::Normal,
            UILayer::Console { .. } => CursorMode::Normal,
        }
    }


    pub fn is_escapable(&self) -> bool {
        match self {
            UILayer::Gameplay => false,
            UILayer::Inventory { .. } => true,
            UILayer::Console { .. } => true,
        }
    }


    pub fn render(&mut self, game: &mut Game, input: &InputManager, renderer: &mut Renderer, dt: f32) {
        match self {
            UILayer::Console { text, backspace_cooldown, timer, cursor, just_opened, offset } => {
                const TEXT_SIZE : f32 = 0.5;
                let window = renderer.window_size();
                let text_box = Vec2::new(window.x * 0.6, renderer.biggest_y_size * 0.6);
                let box_pos = Vec2::new(0.0, window.y - text_box.y * 0.95);
                renderer.draw_rect(box_pos, text_box, Vec4::new(0.1, 0.1, 0.1, 0.5));
                let text_pos = Vec2::new(box_pos.x, box_pos.y);
                renderer.draw_text(&text, text_pos, TEXT_SIZE, Vec4::ONE);

                for key in input.current_chars() {
                    if !key.is_ascii() {
                        text.insert(*cursor as usize, '?');
                    } else {
                        text.insert(*cursor as usize, *key);
                    }
                    *cursor += 1;
                }

                *timer -= dt;

                if input.is_key_just_pressed(Key::Backspace)
                    || input.is_key_just_pressed(Key::Left)
                    || input.is_key_just_pressed(Key::Right)
                    || input.should_paste_now() {

                    *timer = 0.0;
                    *offset = 1;
                } else if input.is_key_just_pressed(Key::Up) {
                    *timer = 0.0;
                }

                else if input.is_key_pressed(Key::Backspace) {
                    while *timer <= 0.0 {
                        *backspace_cooldown = (*backspace_cooldown * 0.8).max(0.03);
                        *timer += *backspace_cooldown;

                        if input.is_super_pressed() {
                            for _ in 0..*cursor as usize {
                                text.remove(0);
                            }

                            *cursor = 0;

                        } else if input.is_alt_pressed() {
                            let prev = &text[0..*cursor as usize];
                            let (word, _) = prev.trim_end().bytes().enumerate().rev().find(|x| x.1 == b' ').unwrap_or((0, 0));
                            let diff = prev.len() - word;
                            for _ in word..prev.len() {
                                text.remove(word);
                            }

                            *cursor -= diff as u32;

                        } else {
                            if *cursor > 0 {
                                text.remove(*cursor as usize - 1);
                            }
                            *backspace_cooldown = (*backspace_cooldown * 0.8).max(0.03);
                            *timer += *backspace_cooldown;
                            if *cursor > 0 {
                                *cursor -= 1;
                            }
                        }
                    }
                } 
                else if input.should_paste() {
                    if let Some(cb) = renderer.window.get_clipboard_string() {
                        while *timer <= 0.0 {
                            *backspace_cooldown = (*backspace_cooldown * 0.8).max(0.03);
                            *timer += *backspace_cooldown;
                            for ch in cb.chars() {
                                if ch == '\n' { continue }
                                if !ch.is_ascii() {
                                    text.insert(*cursor as usize, '?');
                                } else {
                                    text.insert(*cursor as usize, ch);
                                }
                                *cursor += 1;
                            }
                        }
                    }
                }
                else if input.is_key_pressed(Key::Left) {
                    while *timer <= 0.0 {
                        *backspace_cooldown = (*backspace_cooldown * 0.8).max(0.03);
                        *timer += *backspace_cooldown;

                        if input.is_super_pressed() {
                            *cursor = 0;

                        } else if input.is_alt_pressed() {
                            let prev = &text[0..*cursor as usize];
                            let word = prev.trim_end().bytes().enumerate().rev().find(|x| x.1 == b' ')
                                .map(|(i, _)| i + 1).unwrap_or(0);
                            *cursor = word as u32;

                        } else if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                }
                else if input.is_key_pressed(Key::Right) {
                    while *timer <= 0.0 {
                        *backspace_cooldown = (*backspace_cooldown * 0.8).max(0.03);
                        *timer += *backspace_cooldown;

                        if input.is_super_pressed() {
                            *cursor = text.len() as u32;

                        } else if input.is_alt_pressed() {
                            let next = &text[*cursor as usize..];
                            let (word, _) = next.bytes().enumerate().skip_while(|x| x.1 == b' ').find(|x| x.1 == b' ')
                                .unwrap_or((next.len(), 0));
                            *cursor += word as u32;

                        } else if *cursor < text.len() as u32 {
                            *cursor += 1;
                        }
                    }
                }

                else {
                    *backspace_cooldown = 0.5;
                    *timer = *backspace_cooldown;
                }

                let cursor_pos = Vec2::new(text_pos.x + renderer.text_size(&text[0..*cursor as usize], TEXT_SIZE).x, text_pos.y + renderer.biggest_y_size * 0.075);
                renderer.draw_rect(cursor_pos, Vec2::new(renderer.biggest_y_size * 0.05, renderer.biggest_y_size * 0.45), Vec4::ONE);

                if input.is_key_pressed(Key::Up) {
                    while *timer <= 0.0 {
                        *backspace_cooldown = (*backspace_cooldown * 0.8).max(0.03);
                        *timer += *backspace_cooldown;

                        let history = &game.command_registry.previous_commands;
                        if history.len() >= *offset as usize && let Some(cmd) = history.get(history.len() - *offset as usize) {
                            text.clear();
                            text.extend(cmd.as_str().chars());
                            *cursor = text.len() as u32;
                            *offset += 1;
                        }
                    }
                }



                if input.is_key_just_pressed(Key::Enter) && !*just_opened {
                    if !text.is_empty() {
                        let command = Command::parse(core::mem::take(text));
                        game.call_command(command);
                    }

                    *self = UILayer::Gameplay;
                } else {
                    *just_opened = false;
                }
                
            }


            UILayer::Inventory { just_opened } => {
                let window = renderer.window_size();
                renderer.draw_rect(Vec2::ZERO, window, Vec4::new(0.1, 0.1, 0.1, 0.5));
                if input.is_key_just_pressed(Key::E) && !*just_opened {
                    *self = UILayer::Gameplay;
                } else {
                    *just_opened = false;
                }

            }

            UILayer::Gameplay => {
                // render debug text
                {
                    let mut text = String::new();

                    let fps = (1.0 / dt).round();
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
                                StructureData::Quarry { current_progress, output } => {
                                    let _ = writeln!(text, "Quarry:");
                                    let _ = writeln!(text, "§e    - CURRENT PROGRESS: §a{}", current_progress);
                                    if let Some(output) = output {
                                        let _ = writeln!(text, "§e  - OUTPUT: §b{:?}", output);
                                    } else {
                                        let _ = writeln!(text, "§e  - OUTPUT: §bEmpty");
                                    }
                                },

                                StructureData::Inserter { state, filter } => {
                                    let _ = writeln!(text, "Inserter:");
                                    if let Some(filter) = filter {
                                        let _ = writeln!(text, "§e  - FILTER: §a{filter:?}");
                                    } else {
                                        let _ = writeln!(text, "§e  - FILTER: §aNone");
                                    }


                                    match state {
                                        InserterState::Searching => {
                                            let _ = writeln!(text, "§e  - STATE: §aSearching");
                                        },
                                        InserterState::Placing(item) => {
                                            let _ = writeln!(text, "§e  - STATE: §bPlacing");
                                            let _ = writeln!(text, "§e    - ITEM: §b{:?}", item);
                                        },
                                    }
                                },


                                StructureData::Chest { inventory } => {
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


                                StructureData::Belt { inventory } => {
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

                        let mut i = 0;
                        for dropped_item in game.world.dropped_items.iter() {
                            let _ = writeln!(text, "§e- §b{:?}§e: §a{:.1}, {:.1}, {:.1}", dropped_item.item, dropped_item.body.position.x, dropped_item.body.position.y, dropped_item.body.position.z);
                            i += 1;
                            if i > 3 {
                                let len = game.world.dropped_items.len();
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

                    renderer.draw_text(&text, Vec2::ZERO, 0.4, Vec4::ONE);
                }
            },
        }
    }
}
