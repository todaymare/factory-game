use std::{fmt::Write, ops::Bound};

use glam::{Vec2, Vec3, Vec4};
use glfw::{get_key_name, get_key_scancode, CursorMode, Key};
use rand::seq::IndexedRandom;

use crate::{commands::Command, crafting::{Recipe, RECIPES}, input::InputManager, items::{DroppedItem, Item, ItemKind}, renderer::{point_in_rect, Renderer}, structures::strct::{InserterState, StructureData}, voxel_world::split_world_pos, Game, Player, PLAYER_HOTBAR_SIZE, PLAYER_REACH, PLAYER_ROW_SIZE};

pub enum UILayer {
    Inventory {
        just_opened: bool,
        holding_item: Option<Item>,
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


pub const HOTBAR_KEYS : &[Key] = &[Key::Num1, Key::Num2, Key::Num3,
                               Key::Num4, Key::Num5, Key::Num6];


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


            UILayer::Inventory { just_opened, holding_item } => {
                let window = renderer.window_size();
                if input.is_key_just_pressed(Key::E) && !*just_opened {
                    if let Some(holding_item) = holding_item {
                        game.world.dropped_items.push(DroppedItem::new(*holding_item, game.player.body.position));
                    }

                    *self = UILayer::Gameplay;
                    return;
                } else {
                    *just_opened = false;
                }


                renderer.draw_rect(Vec2::ZERO, window, Vec4::new(0.1, 0.1, 0.1, 0.5));
                let window = renderer.window_size();

                let rows = PLAYER_HOTBAR_SIZE;
                let cols = PLAYER_ROW_SIZE;

                let slot_size = 64.0;
                let padding = 16.0;

                let size = Vec2::new(cols as f32, rows as f32) * (slot_size + padding) as f32;
                let mut corner = window * 0.5 - size * 0.5;
                corner.x += size.x * 0.5;
                corner.x += 8.0;
                draw_recipes(renderer, game, input, holding_item, corner);

                corner.x -= size.x;
                corner.x -= 16.0;

                draw_inventory(renderer, game, input, holding_item, corner);
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


                    if !game.structures.work_queue.entries.is_empty() {
                        let _ = writeln!(text, "§eCRAFT QUEUE:");

                        let mut i = 0;
                        let mut total = 0;
                        for (item, ticks) in game.craft_queue.iter() {
                            total += *ticks;
                            let _ = writeln!(text, "§e- §b{:?}§e in §a{} §eticks", item, (total - game.craft_progress));
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



fn draw_recipes(renderer: &mut Renderer, game: &mut Game, input: &InputManager, holding_item: &mut Option<Item>, corner: Vec2) {
    let rows = PLAYER_HOTBAR_SIZE;
    let cols = PLAYER_ROW_SIZE;

    let slot_size = 64.0;
    let padding = 16.0;

    let size = Vec2::new(cols as f32, rows as f32) * (slot_size + padding) as f32;

    renderer.draw_rect(corner, size, Vec4::ONE);

    let mut base = corner + padding * 0.5;
    let point = renderer.to_point(input.mouse_position());
    for col in 0..cols {
        let mut pos = base;
        for row in 0..rows {
            // render
            let Some(&recipe) = RECIPES.get(col*rows+row)
            else { return };

            let can_craft = can_craft(recipe, &game.player);
            let is_mouse_intersecting = point_in_rect(point, pos, Vec2::splat(slot_size));

            if is_mouse_intersecting && can_craft && input.is_button_just_pressed(glfw::MouseButton::Button1) {
                for required_item in recipe.requirements.iter() {
                    let mut needed_amount = required_item.amount;
                    for slot in &mut game.player.inventory {
                        let Some(item) = slot
                        else { continue };

                        if required_item.kind != item.kind { continue }

                        let take = item.amount.min(needed_amount).min(item.kind.max_stack_size());
                        item.amount -= take;
                        if item.amount == 0 {
                            *slot = None;

                        }

                        needed_amount -= take;
                        if needed_amount == 0 { break }
                    }
                }

                game.craft_queue.push((recipe.result, recipe.time));
            }

            let mut colour = if can_craft { Vec4::new(0.2, 0.6, 0.2, 1.0) }
                         else { Vec4::new(0.6, 0.2, 0.2, 1.0) }; 
            if is_mouse_intersecting {
                colour += Vec4::splat(0.4);
            }
           
            renderer.draw_rect(pos, Vec2::splat(slot_size), colour);
            renderer.draw_item_icon(recipe.result.kind, pos+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
            renderer.draw_text(format!("{}", recipe.result.amount).as_str(), pos+slot_size*0.05, 0.5, Vec4::ONE);


            if is_mouse_intersecting {
                let size = Vec2::new(recipe.requirements.len() as f32, 1.0) * (padding + slot_size);
                renderer.draw_rect(point, size, Vec4::new(0.2, 0.2, 0.2, 1.0));
                let mut base = point + padding*0.5;
                for item in recipe.requirements {
                    let available_items = items_in_array(&game.player.inventory, item.kind);
                    let can_craft = available_items >= item.amount;
                    let colour = if can_craft { Vec4::new(0.2, 0.6, 0.2, 1.0) }
                                 else { Vec4::new(0.6, 0.2, 0.2, 1.0) }; 

                    renderer.draw_rect(base, Vec2::splat(slot_size), colour);
                    renderer.draw_item_icon(item.kind, base+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
                    renderer.draw_text(format!("{}/{}", available_items, item.amount).as_str(), base+slot_size*0.05, 0.4, Vec4::ONE);
                    base += Vec2::new(slot_size+padding, 0.0);
                }
            }
            pos += Vec2::new(0.0, slot_size+padding);
        }
        base += Vec2::new(slot_size+padding, 0.0)
    }
}


fn can_craft(recipe: Recipe, player: &Player) -> bool {
    for required_item in recipe.requirements.iter() {
        let available_amount = items_in_array(&player.inventory, required_item.kind);
        if available_amount >= required_item.amount { continue }
        return false;
    }

    true
}


fn items_in_array(arr: &[Option<Item>], kind: ItemKind) -> u32 {
    let mut available_amount = 0;
    for slot in arr {
        let Some(slot) = slot
        else { continue };

        if kind != slot.kind { continue }

        available_amount += slot.amount;
    }

    available_amount
}


fn draw_inventory(renderer: &mut Renderer, game: &mut Game, input: &InputManager, holding_item: &mut Option<Item>, corner: Vec2) {
    let rows = PLAYER_HOTBAR_SIZE;
    let cols = PLAYER_ROW_SIZE;

    let slot_size = 64.0;
    let padding = 16.0;

    let size = Vec2::new(cols as f32, rows as f32) * (slot_size + padding) as f32;

    renderer.draw_rect(corner, size, Vec4::ONE);

    let mut base = corner + padding * 0.5;
    let point = renderer.to_point(input.mouse_position());
    for col in 0..cols {
        let mut pos = base;
        for row in 0..rows {
            // render
            let is_mouse_intersecting = point_in_rect(point, pos, Vec2::splat(slot_size));

            let colour = if is_mouse_intersecting { Vec4::new(1.0, 0.0, 0.0, 1.0) }
                         else if col == game.player.hotbar { Vec4::new(0.4, 0.6, 0.4, 1.0) }
                         else { (Vec4::ONE * 0.2).with_w(1.0) }; 
            let mut slot = &mut game.player.inventory[col*rows+row];

            renderer.draw_rect(pos, Vec2::splat(slot_size), colour);
            if let Some(item) = *slot {
                renderer.draw_item_icon(item.kind, pos+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
                renderer.draw_text(format!("{}", item.amount).as_str(), pos+slot_size*0.05, 0.5, Vec4::ONE);
            }

            pos += Vec2::new(0.0, slot_size+padding);


            // handle interaction
            if !is_mouse_intersecting {
                continue
            }

            if input.is_button_just_pressed(glfw::MouseButton::Button1) {
                if let Some(inv_item) = slot && let Some(item) = holding_item && inv_item.kind == item.kind {
                    let addition = item.amount.min(inv_item.kind.max_stack_size().max(inv_item.amount) - inv_item.amount);

                    inv_item.amount += addition;

                    item.amount -= addition;
                    if item.amount == 0 {
                        *holding_item = None;
                    }
                    continue
                }

                let item = *slot;
                *slot = *holding_item;
                *holding_item = item;
            }
            else if input.is_button_just_pressed(glfw::MouseButton::Button2) {
                if let Some(item) = slot && holding_item.is_none() {
                    let amount = item.amount;
                    item.amount -= amount / 2;

                    let mut new_item = *item;
                    new_item.amount = amount / 2;
                    if new_item.amount != 0 {
                        *holding_item = Some(new_item);
                        continue;
                    }
                }

                let item = *slot;
                *slot = *holding_item;
                *holding_item = item;
            }
            else {
                for (i, &key) in HOTBAR_KEYS.iter().enumerate() {
                    if !input.is_key_just_pressed(key) { continue }

                    let slot_item = *slot;

                    let offset = game.player.hotbar * PLAYER_HOTBAR_SIZE;
                    let item = game.player.inventory[offset+i];
                    game.player.inventory[offset+i] = slot_item;
                    slot = &mut game.player.inventory[col*rows+row];
                    *slot = item;
                }
            }
        }

        base += Vec2::new(slot_size+padding, 0.0)
    }


    if let Some(item) = *holding_item {
        renderer.draw_item_icon(item.kind, point, Vec2::splat(slot_size), Vec4::ONE);
        renderer.draw_text(format!("{}", item.amount).as_str(), point+slot_size*0.05, 0.5, Vec4::ONE);
    }


}
