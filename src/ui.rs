use std::{ops::Bound, fmt::Write};

use glam::{DVec3, Vec2, Vec4};
use glfw::{CursorMode, Key};

use crate::{commands::Command, crafting::{self, Recipe, RECIPES}, input::InputManager, items::{DroppedItem, Item, ItemKind}, renderer::{point_in_rect, Renderer}, structures::{self, inventory::{SlotKind, StructureInventory}, strct::{InserterState, StructureData}, StructureId}, voxel_world::{split_world_pos, VoxelWorld}, Game, Player, PLAYER_HOTBAR_SIZE, PLAYER_INVENTORY_SIZE, PLAYER_REACH, PLAYER_ROW_SIZE, TICKS_PER_SECOND};

pub enum UILayer {
    Inventory {
        just_opened: bool,
        holding_item: Option<Item>,
        inventory_mode: InventoryMode,
    },
    Console {
        text: String,
        backspace_cooldown: f32,
        timer: f32,
        cursor: u32,
        just_opened: bool,
        offset: u32,
    },
    Gameplay { smoothed_dt: f32 },
    None,
}


pub enum InventoryMode {
    Chest(StructureId),
    Silo(StructureId),
    Assembler(StructureId),
    Recipes,
}


pub const HOTBAR_KEYS : &[Key] = &[Key::Num1, Key::Num2, Key::Num3,
                               Key::Num4, Key::Num5];
const SLOT_SIZE : f32 = 64.0;


impl UILayer {
    pub fn inventory_view(mode: InventoryMode) -> Self {
        Self::Inventory { just_opened: true, holding_item: None, inventory_mode: mode }
    }


    pub fn capture_mode(&self) -> CursorMode {
        match self {
            UILayer::Gameplay { .. } => CursorMode::Disabled,
            UILayer::Inventory { .. } => CursorMode::Normal,
            UILayer::Console { .. } => CursorMode::Normal,
            UILayer::None => CursorMode::Normal,
        }
    }


    pub fn is_escapable(&self) -> bool {
        match self {
            UILayer::Gameplay { .. } => false,
            UILayer::Inventory { .. } => true,
            UILayer::Console { .. } => true,
            UILayer::None => false,
        }
    }


    pub fn close(&mut self, game: &mut Game, dt: f32) {
        match self {
            UILayer::Inventory { holding_item, .. } => {
                if let Some(holding_item) = holding_item {
                    game.world.drop_item(*holding_item, game.player.body.position);
                }

                *self = UILayer::Gameplay { smoothed_dt: dt };
            },


            UILayer::Console { .. } => {
                *self = UILayer::Gameplay { smoothed_dt: dt };
            },


            UILayer::Gameplay { .. } => (),


            UILayer::None => (),
        }
    }


    pub fn render(&mut self, game: &mut Game, input: &InputManager, dt: f32) {
        match self {
            UILayer::Console { text, backspace_cooldown, timer, cursor, just_opened, offset } => {
                const TEXT_SIZE : f32 = 0.5;
                let window = game.renderer.window_size();
                let text_box = Vec2::new(window.x * 0.6, game.renderer.biggest_y_size * 0.6);
                let box_pos = Vec2::new(0.0, window.y - text_box.y * 0.95);
                game.renderer.draw_rect(box_pos, text_box, Vec4::new(0.1, 0.1, 0.1, 0.5));
                let text_pos = Vec2::new(box_pos.x, box_pos.y);
                game.renderer.draw_text(&text, text_pos, TEXT_SIZE, Vec4::ONE);

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
                    if let Some(cb) = game.renderer.window.get_clipboard_string() {
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

                let cursor_pos = Vec2::new(text_pos.x + game.renderer.text_size(&text[0..*cursor as usize], TEXT_SIZE).x, text_pos.y + game.renderer.biggest_y_size * 0.075);
                game.renderer.draw_rect(cursor_pos, Vec2::new(game.renderer.biggest_y_size * 0.05, game.renderer.biggest_y_size * 0.45), Vec4::ONE);

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

                    self.close(game, dt);
                } else {
                    *just_opened = false;
                }
                
            }


            UILayer::Inventory { just_opened, holding_item, inventory_mode } => {
                let window = game.renderer.window_size();
                if input.is_key_just_pressed(Key::E) && !*just_opened {
                    self.close(game, dt);
                    return;
                } else {
                    *just_opened = false;
                }


                game.renderer.draw_rect(Vec2::ZERO, window, Vec4::new(0.1, 0.1, 0.1, 0.5));
                let window = game.renderer.window_size();

                let rows = PLAYER_ROW_SIZE;
                let cols = PLAYER_HOTBAR_SIZE;

                let slot_size = 64.0;
                let padding = 16.0;

                let player_inv_size = Vec2::new(cols as f32, rows as f32) * (slot_size + padding) as f32;
                let mut other_inv = None;

                'mode: {
                match inventory_mode {
                    InventoryMode::Chest(structure) => {
                        let rows = 3;
                        let cols = 3;
                        let external_view_size = Vec2::new(rows as f32, cols as f32) * (slot_size + padding) as f32;

                        let mut corner = window * 0.5 - external_view_size * 0.5;
                        corner.x += external_view_size.x * 0.5;
                        corner.x += padding * 0.5;


                        let structure = game.structures.get_mut(*structure);
                        let inventory = &mut structure.inventory.as_mut().unwrap().slots;

                        game.renderer.draw_rect(corner, external_view_size, Vec4::ONE);
                        draw_inventory(&mut game.renderer, &mut *inventory, game.player.body.position, &mut game.world, Some(&mut game.player.inventory), input, holding_item, corner, cols, rows);

                        other_inv = Some(inventory.as_mut_slice());
                    },


                    InventoryMode::Silo(structure) => {
                        let rows = 6;
                        let cols = 6;
                        let external_view_size = Vec2::new(rows as f32, cols as f32) * (slot_size + padding) as f32;

                        let mut corner = window * 0.5 - external_view_size * 0.5;
                        corner.x += external_view_size.x * 0.5;
                        corner.x += padding * 0.5;


                        let structure = game.structures.get_mut(*structure);
                        let inventory = &mut structure.inventory.as_mut().unwrap().slots;

                        game.renderer.draw_rect(corner, external_view_size, Vec4::ONE);
                        draw_inventory(&mut game.renderer, inventory, game.player.body.position, &mut game.world, Some(&mut game.player.inventory), input, holding_item, corner, cols, rows);

                        other_inv = Some(inventory.as_mut_slice());
                    },


                    InventoryMode::Assembler(structure) => {
                        let mut corner = window * 0.5 - player_inv_size * 0.5;
                        corner.x += player_inv_size.x * 0.5;
                        corner.x += padding * 0.5;

                        let rows = PLAYER_HOTBAR_SIZE;
                        let cols = PLAYER_ROW_SIZE;

                        let size = Vec2::new(rows as f32, cols as f32) * (slot_size + padding) as f32;

                        game.renderer.draw_rect(corner, size, Vec4::ONE);

                        let mut base = corner + padding * 0.5;
                        let point = game.renderer.to_point(input.mouse_position());
                        for col in 0..cols {
                            let mut pos = base;
                            for row in 0..rows {
                                let recipe_index = col*rows+row;
                                let Some(&curr_recipe) = RECIPES.get(recipe_index)
                                else { break 'mode };

                                let is_mouse_intersecting = point_in_rect(point, pos, Vec2::splat(slot_size));
                                let mut colour = (Vec4::ONE * 0.2).with_w(1.0); 

                                if is_mouse_intersecting {
                                    colour += Vec4::splat(0.4);
                                }
                               
                                game.renderer.draw_rect(pos, Vec2::splat(slot_size), colour);
                                game.renderer.draw_item_icon(curr_recipe.result.kind, pos+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
                                game.renderer.draw_text(format!("{}", curr_recipe.result.amount).as_str(), pos+slot_size*0.05, 0.5, Vec4::ONE);


                                if is_mouse_intersecting && input.is_button_just_pressed(glfw::MouseButton::Button1) {
                                    let structure = game.structures.get_mut(*structure);
                                    let StructureData::Assembler { recipe } = &mut structure.data
                                    else { unreachable!() };

                                    let prev_inv = if recipe.is_some() {
                                        core::mem::take(&mut structure.inventory.as_mut().unwrap().slots)
                                    } else { vec![] };


                                    let new_inventory_slots = crafting::crafting_recipe_inventory(recipe_index);
                                    let new_inv = StructureInventory::new(new_inventory_slots);

                                    structure.inventory = Some(new_inv);
                                    *recipe = Some(curr_recipe);

                                    for item in prev_inv {
                                        let Some(item) = item
                                        else { continue };

                                        if structure.can_accept(item) {
                                            structure.give_item(item);
                                        } else {
                                            let dropped_item = DroppedItem::new(item, game.player.body.position);
                                            game.world.dropped_items.push(dropped_item);
                                        }
                                    }

                                    self.close(game, dt);
                                    return;
                                }


                                pos += Vec2::new(slot_size+padding, 0.0);
                            }

                            base += Vec2::new(0.0, slot_size+padding);

                        }

                    }


                    InventoryMode::Recipes => {

                        let mut corner = window * 0.5 - player_inv_size * 0.5;
                        corner.x += player_inv_size.x * 0.5;
                        corner.x += padding * 0.5;

                        draw_recipes(game, input, holding_item, corner);
                    },
                }
                }

                let mut corner = window * 0.5 - player_inv_size * 0.5;
                corner.x -= player_inv_size.x * 0.5;
                corner.x -= padding * 0.5;

                draw_player_inventory(&mut game.renderer, &mut game.player, &mut game.world, &mut other_inv, input, holding_item, corner);
            }

            UILayer::Gameplay { smoothed_dt } => {
                // render debug text
                {
                    let mut text = String::new();

                    let alpha = 0.1;
                    *smoothed_dt = (1.0 - alpha) * *smoothed_dt + alpha * dt;
                    let fps = (1.0 / *smoothed_dt).round();
                    let colour_code = if fps > 55.0 { 'a' } else if fps > 25.0 { '6' } else { '4' };

                    let _ = writeln!(text, "§eFPS: §{colour_code}{fps}§r");
                    let _ = writeln!(text, "§eTIME ELAPSED: §a{:.1}§r", game.current_tick.u32() as f64 / TICKS_PER_SECOND as f64);
                    let _ = writeln!(text, "§eDRAW CALLCOUNT: §a{}§r", game.renderer.draw_count.get());
                    let _ = writeln!(text, "§eTRIANGLE COUNT: §a{}§r", game.renderer.triangle_count.get());
                    game.renderer.triangle_count.set(0);
                    game.renderer.draw_count.set(0);

                    let _ = writeln!(text, "§eRENDER WORLD TIME: §a{}ms§r", game.render_world_time);
                    let _ = writeln!(text, "§eRENDERED CHUNKS: §a{}§r", game.total_rendered_chunks);

                    let _ = writeln!(text, "§ePITCH: §a{:.1}({:.1}) §eYAW: §a{:.1}({:.1})§r", game.camera.pitch.to_degrees(), game.camera.pitch, game.camera.yaw.to_degrees(), game.camera.yaw);
                    let _ = writeln!(text, "§ePOSITION: §a{:.1}, {:.1} {:.1}§r", game.camera.position.x, game.camera.position.y, game.camera.position.z);

                    let (chunk_pos, chunk_local_pos) = split_world_pos(game.player.body.position.floor().as_ivec3());
                    let _ = writeln!(text, "§eCHUNK POSITION: §a{}, {}, {}§r", chunk_pos.x, chunk_pos.y, chunk_pos.z);
                    let _ = writeln!(text, "§eCHUNK LOCAL POSITION: §a{}, {}, {}§r", chunk_local_pos.x, chunk_local_pos.y, chunk_local_pos.z);
                    let _ = writeln!(text, "§eCHUNK COUNT: §a{}§r", game.world.chunks.len());
                    let _ = writeln!(text, "§eDIRECTION: §b{:?}§r", game.camera.compass_direction());


                    let target_block = game.world.raycast_voxel(game.camera.position, game.camera.front, PLAYER_REACH);
                    if let Some(target_block) = target_block {
                        let target_voxel = game.world.get_voxel(target_block.0);
                        let target_voxel_kind = target_voxel;


                        let _ = writeln!(text, "§eTARGET LOCATION: §a{}, {}, {}", target_block.0.x, target_block.0.y, target_block.0.z);


                        let _ = write!(text, "§eTARGET BLOCK: §b");


                        if target_voxel.is_structure() {
                            let structure = game.world.structure_blocks.get(&target_block.0).unwrap();
                            let structure = game.structures.get(*structure);

                            let _ = writeln!(text, "Structure");
                            let _ = writeln!(text, "§e- POSITION: §a{}, {}, {}", structure.position.x, structure.position.y, structure.position.z);
                            let _ = writeln!(text, "§e- DIRECTION: §b{:?}", structure.direction);
                            let _ = writeln!(text, "§e- IS ASLEEP: §b{}", structure.is_asleep);

                            if let Some(inv) = &structure.inventory {
                                let input_len = inv.inputs_len();
                                if input_len > 0 {
                                    let _ = writeln!(text, "§e  - INPUTS:");
                                    for i in 0..input_len {
                                        let (item, meta) = inv.input(i);
                                        let filter = match meta.kind {
                                            SlotKind::Input { filter } => filter,
                                            SlotKind::Storage => None,
                                            SlotKind::Output => unreachable!(),
                                        };

                                        if let Some(item) = item {
                                            let max_amount = meta.max_amount.min(item.kind.max_stack_size());
                                            let _ = writeln!(text, "§e     - §b{:?} §a{}x/{}x", item.kind, item.amount, max_amount);
                                        } else if let Some(filter) = filter && meta.max_amount != u32::MAX {
                                            let max_amount = meta.max_amount;
                                            let _ = writeln!(text, "§e     - §b{:?} §a0x/{}x", filter, max_amount);
                                        } else {
                                            let _ = writeln!(text, "§e     - §bEmpty");
                                        }
                                    }
                                }

                                let output_len = inv.outputs_len();
                                let _ = writeln!(text, "§e  - OUTPUTS:");
                                for i in 0..output_len {
                                    let (item, meta) = inv.output(i);
                                    if let Some(item) = item {
                                        let max_amount = meta.max_amount.min(item.kind.max_stack_size());
                                        let _ = writeln!(text, "§e     - §b{:?} §a{}x/{}x", item.kind, item.amount, max_amount);
                                    } else {
                                        let _ = writeln!(text, "§e     - §bEmpty");
                                    }
                                }
                            }

                            let _ = write!(text, "§e- KIND: §b");

                            match &structure.data {
                                StructureData::Quarry { current_progress } => {
                                    let _ = writeln!(text, "Quarry:");
                                    let _ = writeln!(text, "§e    - CURRENT PROGRESS: §a{}", current_progress);
                                    let y = *current_progress / 9;
                                    let y = structure.zero_zero().y + -(y as i32) - 1;
                                    let eff = structures::quarry_efficiency(y as _);
                                    let _ = writeln!(text, "§e    - EFFICIENCY: §a{:.1}%", (1.0 / eff) * 100.0);
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


                                StructureData::Chest { } => {
                                    let _ = writeln!(text, "Chest");
                                }


                                StructureData::Silo { } => {
                                    let _ = writeln!(text, "Silo");
                                }


                                StructureData::Belt { } => {
                                    let _ = writeln!(text, "Belt");
                                }


                                StructureData::Splitter { priority } => {
                                    let _ = writeln!(text, "Splitter");
                                    let _ = writeln!(text, "§e  - PRIORITY: §a{priority:?}");
                                }


                                StructureData::Assembler { recipe: crafter } => {
                                    let _ = writeln!(text, "Assembler");
                                    let _ = writeln!(text, "§e  - RECIPE: §a{crafter:?}");
                                }

                                StructureData::Furnace { input, output } => {
                                    let _ = writeln!(text, "Furnace");
                                    let _ = writeln!(text, "§e  - INPUT: §b{input:?}");
                                    let _ = writeln!(text, "§e  - OUTPUT: §b{output:?}");
                                }
                            }
                        } else {
                           let _ = writeln!(text, "{:?}", target_voxel);
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


                    if !game.craft_queue.is_empty() {
                        let _ = writeln!(text, "§eCRAFT QUEUE:");

                        let mut i = 0;
                        let mut total = 0;
                        for (item, ticks) in game.craft_queue.iter() {
                            total += *ticks;
                            let _ = writeln!(text, "§e- §b{:?}§e in §a{} §eticks", item, (total - game.craft_progress));
                            i += 1;
                            if i > 3 && i < game.craft_queue.len() {
                                let len = game.craft_queue.len();
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
                            if i > 3 && i < game.world.dropped_items.len() {
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

                    game.renderer.draw_text(&text, Vec2::ZERO, 0.4, Vec4::ONE);
                }
            },


            UILayer::None => unreachable!(),
        }
    }
}



fn draw_recipes(game: &mut Game, input: &InputManager, _: &mut Option<Item>, corner: Vec2) {
    let rows = PLAYER_HOTBAR_SIZE;
    let cols = PLAYER_ROW_SIZE;

    let slot_size = 64.0;
    let padding = 16.0;

    let size = Vec2::new(rows as f32, cols as f32) * (slot_size + padding) as f32;

    game.renderer.draw_rect(corner, size, Vec4::ONE);

    let mut base = corner + padding * 0.5;
    let point = game.renderer.to_point(input.mouse_position());
    for col in 0..cols {
        let mut pos = base;
        for row in 0..rows {
            // render
            let Some(&recipe) = RECIPES.get(col*rows+row)
            else { return };

            let (can_craft, mut rc) = RecipeCraft::try_craft(game.player.inventory, recipe);
            let is_mouse_intersecting = point_in_rect(point, pos, Vec2::splat(slot_size));

            if is_mouse_intersecting && can_craft && input.is_button_just_pressed(glfw::MouseButton::Button1) {
                game.player.inventory = rc.inv;
                assert!(can_craft);

                for step in rc.craft_queue.iter().rev() {
                    let CraftStepResult::Craftable(recipe) = step.result
                    else { continue };

                    let mut item = recipe.result;
                    let item_in_buffer = rc.buffer.iter_mut().find(|x| x.kind == step.item);

                    if let Some(item_in_buffer) = item_in_buffer && item_in_buffer.amount > 0 {
                        let diff = item_in_buffer.amount.min(item.amount);
                        let overflow = (item.amount - diff).min(item_in_buffer.amount);
                        item_in_buffer.amount -= overflow;
                        item.amount = overflow;
                    } else if step.depth != 0 {
                        item.amount = 0;
                    }

                    game.craft_queue.push((item, recipe.time*step.amount));
                }
            }

            let mut colour = if can_craft { Vec4::new(0.2, 0.6, 0.2, 1.0) }
                             else { Vec4::new(0.6, 0.2, 0.2, 1.0) }; 

            if is_mouse_intersecting {
                colour += Vec4::splat(0.4);
            }
           
            game.renderer.draw_rect(pos, Vec2::splat(slot_size), colour);
            game.renderer.draw_item_icon(recipe.result.kind, pos+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
            game.renderer.draw_text(format!("{}", recipe.result.amount).as_str(), pos+slot_size*0.05, 0.5, Vec4::ONE);


            if is_mouse_intersecting {
                let size = Vec2::new(recipe.requirements.len() as f32, 1.0) * (padding + slot_size);
                game.renderer.with_z(1.0, |renderer| {
                renderer.draw_rect(point, size, Vec4::new(0.2, 0.2, 0.2, 1.0));
                let mut base = point + padding*0.5;
                for item in recipe.requirements.iter() {
                    let craft_step = rc.craft_queue.iter()
                        .find(|x| x.item == item.kind && x.depth == 1)
                        .map(|x| x.result)
                        .unwrap();

                    let colour = match craft_step {
                        CraftStepResult::DirectlyAvailable => Vec4::new(0.2, 0.6, 0.2, 1.0),
                        CraftStepResult::Craftable(_) => Vec4::new(0.6, 0.6, 0.2, 1.0),
                        CraftStepResult::NotCraftable => Vec4::new(0.6, 0.2, 0.2, 1.0),
                        CraftStepResult::NotAvailableRawMaterial => Vec4::new(0.6, 0.2, 0.2, 1.0),
                    };

                    renderer.draw_rect(base, Vec2::splat(slot_size), colour);
                    renderer.draw_item_icon(item.kind, base+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
                    renderer.draw_text(format!("{}", item.amount).as_str(), base+slot_size*0.05, 0.4, Vec4::ONE);
                    base += Vec2::new(slot_size+padding, 0.0);
                }

                });
            }
            pos += Vec2::new(slot_size+padding, 0.0);
        }
        base += Vec2::new(0.0, slot_size+padding);
    }
}


#[derive(Clone, Debug)]
struct RecipeCraft {
    inv: [Option<Item>; PLAYER_INVENTORY_SIZE],
    buffer: Vec<Item>,
    craft_queue: Vec<CraftStep>
}


#[derive(Clone, Debug)]
struct CraftStep {
    item: ItemKind,
    depth: u32,
    result: CraftStepResult,
    amount: u32,
}


#[derive(Clone, Copy, Debug)]
enum CraftStepResult {
    DirectlyAvailable,
    Craftable(Recipe),
    NotCraftable,
    NotAvailableRawMaterial,
}


impl RecipeCraft {
    pub fn try_craft(inv: [Option<Item>; PLAYER_INVENTORY_SIZE], recipe: Recipe) -> (bool, RecipeCraft) {
        let mut this = RecipeCraft {
            buffer: vec![],
            craft_queue: vec![],
            inv,
        };

        let result = this.perform_craft(0, recipe, 1);
        (result, this)
    }


    fn perform_craft(&mut self, depth: u32, recipe: Recipe, amount: u32) -> bool {
        let index = self.craft_queue.len();
        let step = CraftStep { item: recipe.result.kind, depth, amount,
                                result: CraftStepResult::NotCraftable };

        self.craft_queue.push(step);

        let mut return_value = true;
        for required_item in recipe.requirements.iter() {
            let needed = required_item.amount * amount;

            let directly_available = self.directly_available(required_item.kind);
            if directly_available >= needed {
                self.remove_item(required_item.kind, needed);

                let step = CraftStep {
                    item: required_item.kind, depth: depth + 1,
                    amount: required_item.amount,
                    result: CraftStepResult::DirectlyAvailable
                };

                self.craft_queue.push(step);
                continue;
            }

            let mut this = self.clone();
            this.remove_item(required_item.kind, directly_available);

            let needed = needed - directly_available;
            let Some(recipe) = RECIPES.iter().find(|f| f.result.kind == required_item.kind)
            else {
                let step = CraftStep {
                    item: required_item.kind, depth: depth + 1,
                    amount: required_item.amount,
                    result: CraftStepResult::NotAvailableRawMaterial
                };

                self.craft_queue.push(step);
                return_value = false;
                continue;
            };

            let recipe_amount = needed.div_ceil(recipe.result.amount);
            if !this.perform_craft(depth + 1, *recipe, recipe_amount) {
                let step = CraftStep {
                    item: required_item.kind, depth: depth + 1,
                    amount: recipe_amount,
                    result: CraftStepResult::NotCraftable
                };

                self.craft_queue.push(step);
                return_value = false;
                continue;
            };

            // send off the overflow
            let mut recipe_result = recipe.result;
            recipe_result.amount = recipe.result.amount * recipe_amount - needed;
            *self = this;
            self.add_item(recipe_result);
        }

        if return_value {
            self.craft_queue[index].result = CraftStepResult::Craftable(recipe);
        }

        return_value
    }


    fn directly_available(&self, kind: ItemKind) -> u32 {
        self.inv.iter().filter_map(|f| *f)
            .chain(self.buffer.iter().copied())
            .filter(|x| x.kind == kind)
            .map(|x| x.amount)
            .sum()
    }


    fn remove_item(&mut self, kind: ItemKind, mut amount: u32) {
        if amount == 0 { return }

        // try to remove from the buffer first
        let mut i = 0;
        while let Some(item) = self.buffer.get_mut(i) {
            if item.kind != kind { i += 1; continue }

            let diff = amount.min(item.amount);
            amount -= diff;
            item.amount -= diff;

            if item.amount == 0 {
                self.buffer.remove(i);
            } else {
                i += 1;
            }

            if amount == 0 {
                return;
            }
        }

        // else, try the inventory
        for slot in self.inv.iter_mut() {
            let Some(item) = slot
            else { continue };

            if item.kind != kind { continue }

            let diff = amount.min(item.amount);
            amount -= diff;
            item.amount -= diff;

            if item.amount == 0 {
                *slot = None;
            }

            if amount == 0 {
                return;
            }
        }

        panic!("not enough items in neither the buffer nor inventory");
    }

    fn add_item(&mut self, item: Item) {
        if item.amount == 0 { return }
        if let Some(slot) = self.buffer.iter_mut().find(|x| x.kind == item.kind) {
            slot.amount += item.amount;
        } else {
            self.buffer.push(item);
        }
    }
}


fn draw_player_inventory(renderer: &mut Renderer, player: &mut Player, world: &mut VoxelWorld, other_inv: &mut Option<&mut [Option<Item>]>, input: &InputManager, holding_item: &mut Option<Item>, corner: Vec2) {
    let rows = PLAYER_ROW_SIZE;
    let cols = PLAYER_HOTBAR_SIZE;

    let slot_size = 64.0;
    let padding = 16.0;

    let size = Vec2::new(cols as f32, rows as f32) * (slot_size + padding) as f32;

    renderer.draw_rect(corner, size, Vec4::ONE);

    let mut base = corner + padding * 0.5;
    let point = renderer.to_point(input.mouse_position());
    for row in 0..rows {
        let mut pos = base;
        for col in 0..cols {
            let slot_index = row*cols+col;
            let is_mouse_intersecting = point_in_rect(point, pos, Vec2::splat(SLOT_SIZE));
            let colour = if slot_index/PLAYER_HOTBAR_SIZE == player.hotbar { Vec4::new(0.4, 0.6, 0.4, 1.0) }
                         else { (Vec4::ONE * 0.2).with_w(1.0) }; 


            draw_inventory_item(renderer, &mut player.inventory, player.body.position, world, other_inv, input, holding_item,
                                pos, slot_index, point, colour);

            pos += Vec2::new(slot_size+padding, 0.0);
                    
            
            if !is_mouse_intersecting {
                continue;
            }

            let mut slot = &mut player.inventory[slot_index];
            for (i, &key) in HOTBAR_KEYS.iter().enumerate() {
                if !input.is_key_just_pressed(key) { continue }

                let slot_item = *slot;

                let offset = player.hotbar * PLAYER_HOTBAR_SIZE;
                let item = player.inventory[offset+i];
                player.inventory[offset+i] = slot_item;
                slot = &mut player.inventory[slot_index];
                *slot = item;
                continue
            }


        }

        base += Vec2::new(0.0, slot_size+padding)
    }


    if let Some(item) = *holding_item {
        renderer.draw_item_icon(item.kind, point, Vec2::splat(slot_size), Vec4::ONE);
        renderer.draw_text(format!("{}", item.amount).as_str(), point+slot_size*0.05, 0.5, Vec4::ONE);
    }


}


fn draw_inventory(renderer: &mut Renderer, inventory: &mut [Option<Item>],
                  player_pos: DVec3, world: &mut VoxelWorld,
                  mut other_inv: Option<&mut [Option<Item>]>,
                  input: &InputManager, holding_item: &mut Option<Item>,
                  corner: Vec2, cols: usize, rows: usize) {
    let slot_size = 64.0;
    let padding = 16.0;

    let mut base = corner + padding * 0.5;
    let point = renderer.to_point(input.mouse_position());
    for row in 0..rows {
        let mut pos = base;
        for col in 0..cols {
            let slot_index = row*cols+col;
            let is_mouse_intersecting = point_in_rect(point, pos, Vec2::splat(SLOT_SIZE));
            let colour = (Vec4::ONE * 0.2).with_w(1.0); 

            draw_inventory_item(renderer, inventory, player_pos, world, &mut other_inv, input, holding_item,
                                pos, slot_index, point, colour);

            pos += Vec2::new(slot_size+padding, 0.0);
            
            if !is_mouse_intersecting {
                continue
            }

        }

        base += Vec2::new(0.0, slot_size+padding)
    }


    if let Some(item) = *holding_item {
        renderer.draw_item_icon(item.kind, point, Vec2::splat(slot_size), Vec4::ONE);
        renderer.draw_text(format!("{}", item.amount).as_str(), point+slot_size*0.05, 0.5, Vec4::ONE);
    }
}



fn draw_inventory_item(renderer: &mut Renderer, inventory: &mut [Option<Item>],
                       player_pos: DVec3, world: &mut VoxelWorld,
                       mut other_inv: &mut Option<&mut [Option<Item>]>,
                       input: &InputManager, holding_item: &mut Option<Item>,
                       pos: Vec2, index: usize, mouse: Vec2, mut colour: Vec4) {

    let is_mouse_intersecting = point_in_rect(mouse, pos, Vec2::splat(SLOT_SIZE));
    if is_mouse_intersecting {
        colour += Vec4::splat(0.4);
    }

    renderer.draw_rect(pos, Vec2::splat(SLOT_SIZE), colour);

    let slot = &mut inventory[index];

    if let Some(item) = *slot {
        renderer.draw_item_icon(item.kind, pos+SLOT_SIZE*0.05, Vec2::splat(SLOT_SIZE*0.9), Vec4::ONE);
        renderer.draw_text(format!("{}", item.amount).as_str(), pos+SLOT_SIZE*0.05, 0.5, Vec4::ONE);
    }

    if !is_mouse_intersecting { return }

    if input.is_button_pressed(glfw::MouseButton::Button1) && input.is_key_pressed(Key::LeftShift) {
        if let Some(inv_item) = slot && let Some(other_inv) = &mut other_inv {
            for slot in other_inv.iter_mut() {
                let Some(item) = slot
                else { continue };

                if item.kind != inv_item.kind {
                    continue;
                }

                let addition = inv_item.amount.min(item.kind.max_stack_size() - item.amount);
                inv_item.amount -= addition;
                item.amount += addition;
                if inv_item.amount != 0 {
                    continue;
                }

                let slot = &mut inventory[index];
                *slot = None;
                return;
            }


            for slot in other_inv.iter_mut() {
                if slot.is_some() { continue }

                if inv_item.amount != 0 {
                    *slot = Some(*inv_item);
                }

                let slot = &mut inventory[index];
                *slot = None;
                return ;
            }

        }

    } else if input.is_button_just_pressed(glfw::MouseButton::Button1) {
        if let Some(inv_item) = slot
           && let Some(item) = holding_item
           && inv_item.kind == item.kind {

            let addition = item.amount.min(inv_item.kind.max_stack_size().max(inv_item.amount) - inv_item.amount);

            inv_item.amount += addition;

            item.amount -= addition;
            if item.amount == 0 {
                *holding_item = None;
            }
            return;
        }

        let item = *slot;
        *slot = *holding_item;
        *holding_item = item;
        return;
    } else if input.is_button_just_pressed(glfw::MouseButton::Button2) {
        if let Some(item) = slot && holding_item.is_none() {
            let amount = item.amount;
            item.amount -= amount / 2;

            let mut new_item = *item;
            new_item.amount = amount / 2;
            if new_item.amount != 0 {
                *holding_item = Some(new_item);
                return;
            }
        }

        let item = *slot;
        *slot = *holding_item;
        *holding_item = item;
        return;
    } else if input.is_key_just_pressed(glfw::Key::Q)
        && let Some(item) = slot {
        let mut drop_item = *item;
        if input.is_alt_pressed() {
            drop_item.amount = 1;
        }

        item.amount -= drop_item.amount;
        if item.amount == 0 {
            *slot = None;
        }

        let dropped_item = DroppedItem::new(drop_item, player_pos);
        world.dropped_items.push(dropped_item);

    }

}
