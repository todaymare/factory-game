use glam::{DVec3, Vec2, Vec4};
use kira::{sound::static_sound::{StaticSoundData, StaticSoundHandle}, Tween};
use sti::hash::hash_map::SlotIdx;
use winit::{event::MouseButton, keyboard::KeyCode};
use std::{fmt::Write, ops::Bound};

use crate::{commands::Command, constants::{COAL_ENERGY_PER_UNIT, COLOUR_ADDITIVE_HIGHLIGHT, COLOUR_DARK_GREY, COLOUR_DENY, COLOUR_GREY, COLOUR_PASS, COLOUR_PLAYER_ACTIVE_HOTBAR, COLOUR_SCREEN_DIM, COLOUR_WARN, COLOUR_WHITE, PLAYER_HOTBAR_SIZE, PLAYER_INVENTORY_SIZE, PLAYER_REACH, PLAYER_ROW_SIZE, TICKS_PER_SECOND, UI_HOVER_ACTION_OFFSET, UI_Z_MAX, UI_Z_MIN}, crafting::{self, Recipe, FURNACE_RECIPES, RECIPES}, entities::{EntityKind, EntityMap}, input::InputManager, items::{self, Item, ItemKind}, renderer::{point_in_rect, Renderer}, structures::{self, inventory::{Filter, SlotKind, SlotMeta, StructureInventory}, strct::{InserterState, StructureData}, StructureId}, voxel_world::{chunker::MeshEntry, split_world_pos, VoxelWorld}, Game, Player};

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

    Credits {
        time: f32,
        audio: StaticSoundHandle,
    },

    None,
}


pub enum InventoryMode {
    Chest(StructureId),
    Furnace(StructureId),
    Silo(StructureId),
    Assembler(StructureId),
    Inserter(StructureId),
    Recipes,
}


pub const HOTBAR_KEYS : &[KeyCode] = &[KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3,
                               KeyCode::Digit4, KeyCode::Digit5];
const SLOT_SIZE : f32 = 64.0;


impl UILayer {
    pub fn inventory_view(mode: InventoryMode) -> Self {
        Self::Inventory { just_opened: true, holding_item: None, inventory_mode: mode }
    }


    pub fn is_mouse_locked(&self) -> bool {
        match self {
            UILayer::Gameplay { .. } => true,
            UILayer::Inventory { .. } => false,
            UILayer::Console { .. } => false,
            UILayer::Credits { .. } => false,
            UILayer::None => false,
        }
    }


    pub fn is_escapable(&self) -> bool {
        match self {
            UILayer::Gameplay { .. } => false,
            UILayer::Inventory { .. } => true,
            UILayer::Console { .. } => true,
            UILayer::Credits { .. } => true,
            UILayer::None => false,
        }
    }


    pub fn close(&mut self, game: &mut Game, dt: f32) {
        match self {
            UILayer::Inventory { holding_item, .. } => {
                if let Some(holding_item) = holding_item {
                    game.entities.spawn(
                        EntityKind::dropped_item(*holding_item),
                        game.player.body.position
                    );
                }

                *self = UILayer::Gameplay { smoothed_dt: dt };
            },


            UILayer::Console { .. } => {
                *self = UILayer::Gameplay { smoothed_dt: dt };
            },


            UILayer::Gameplay { .. } => (),


            UILayer::None => (),


            UILayer::Credits { time, audio } => {
                audio.stop(Tween::default());
                
                *self = UILayer::Gameplay { smoothed_dt: dt };
            }
        }
    }


    pub fn render(&mut self, game: &mut Game, input: &InputManager, renderer: &mut Renderer, dt: f32) {
        match self {
            UILayer::Console { text, backspace_cooldown, timer, cursor, just_opened, offset } => {
                const TEXT_SIZE : f32 = 0.5;
                let window = renderer.window_size();
                let text_box = Vec2::new(window.x * 0.6, renderer.line_size * 0.6);
                let box_pos = Vec2::new(0.0, window.y - text_box.y * 0.95);
                renderer.draw_rect(box_pos, text_box, COLOUR_SCREEN_DIM);

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

                if input.is_key_just_pressed(KeyCode::Backspace)
                    || input.is_key_just_pressed(KeyCode::ArrowLeft)
                    || input.is_key_just_pressed(KeyCode::ArrowRight)
                    || input.should_paste_now() {

                    *timer = 0.0;
                    *offset = 1;
                } else if input.is_key_just_pressed(KeyCode::ArrowUp) {
                    *timer = 0.0;
                }

                else if input.is_key_pressed(KeyCode::Backspace) {
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
                /*
                else if input.should_paste() {
                    if let Some(cb) = renderer.window) {
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
                }*/
                else if input.is_key_pressed(KeyCode::ArrowLeft) {
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
                else if input.is_key_pressed(KeyCode::ArrowRight) {
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

                let cursor_pos = Vec2::new(text_pos.x + renderer.text_size(&text[0..*cursor as usize], TEXT_SIZE).x, text_pos.y + renderer.line_size * 0.075);
                renderer.draw_rect(cursor_pos, Vec2::new(renderer.line_size * 0.05, renderer.line_size * 0.45), Vec4::ONE);

                if input.is_key_pressed(KeyCode::ArrowUp) {
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



                if input.is_key_just_pressed(KeyCode::Enter) && !*just_opened {
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
                let window = renderer.window_size();
                if input.is_key_just_pressed(KeyCode::KeyE) && !*just_opened {
                    self.close(game, dt);
                    return;
                } else {
                    *just_opened = false;
                }


                renderer.with_z(UI_Z_MIN, |renderer| {
                    renderer.draw_rect(Vec2::ZERO, window, COLOUR_SCREEN_DIM);
                });

                let window = renderer.window_size();

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

                        renderer.draw_rect(corner, external_view_size, Vec4::ONE);
                        draw_inventory(renderer, &mut *inventory, game.player.body.position, &mut game.world, &mut game.entities, Some(&mut game.player.inventory), input, holding_item, corner, cols, rows);

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

                        renderer.draw_rect(corner, external_view_size, Vec4::ONE);
                        draw_inventory(renderer, inventory, game.player.body.position, &mut game.world, &mut game.entities, Some(&mut game.player.inventory), input, holding_item, corner, cols, rows);

                        other_inv = Some(inventory.as_mut_slice());
                    },



                    InventoryMode::Inserter(structure_id) => {
                        let mut corner = window * 0.5 - player_inv_size * 0.5;
                        corner.x += player_inv_size.x * 0.5;
                        corner.x += padding * 0.5;

                        let rows = PLAYER_HOTBAR_SIZE;
                        let cols = PLAYER_ROW_SIZE;

                        let size = Vec2::new(rows as f32, cols as f32) * (slot_size + padding) as f32;

                        let text = "\n  \
                              §8Left Click §8to §2set §8filter  \n  \
                              §8Right Click §8any slot to §cremove §8filter  \n\n\
                        ";

                        let text_size = renderer.text_size(text, 0.6);

                        let text_pos = window * 0.5 - Vec2::new(text_size.x * 0.5, text_size.y + player_inv_size.y * 0.5 + padding);
                        renderer.draw_rect(text_pos, text_size, Vec4::ONE);
                        renderer.draw_text(text, text_pos, 0.6, Vec4::new(0.2, 0.2, 0.2, 1.0));
                        renderer.draw_rect(corner, size, Vec4::ONE);

                        let mouse_pos = renderer.to_point(input.mouse_position());

                        let mut base = corner + padding * 0.5;
                        for col in 0..cols {
                            let mut pos = base;
                            for row in 0..rows {
                                let index = col*rows+row;
                                let Some(&curr) = ItemKind::ALL.get(index)
                                else { break 'mode };


                                let mut close = false;
                                draw_inventory_slot(
                                    &mut (game.structures.get_mut(*structure_id), &mut close),
                                    renderer,
                                    input,
                                    pos,
                                    Some(Item::new(curr, 1)),
                                    COLOUR_GREY,
                                    |renderer, _| {
                                        default_hover_action(renderer, mouse_pos, curr);
                                    },

                                    |_, (s, close)| {
                                        let StructureData::Inserter { filter, .. } = &mut s.data
                                        else { unreachable!() };

                                        *filter = Some(curr);

                                        **close = true;
                                    },
                                    |_, (s, close)| {
                                        let StructureData::Inserter { filter, .. } = &mut s.data
                                        else { unreachable!() };


                                        *filter = None;
                                        **close = true;
                                    },
                                    |_, _| {},
                                );

                                if close {
                                    self.close(game, dt);
                                    return;
                                }

                                pos += Vec2::new(slot_size+padding, 0.0);
                            }

                            base += Vec2::new(0.0, slot_size+padding);
                        }
                    }




                    InventoryMode::Furnace(structure_id) => {
                        let mut corner = window * 0.5 - player_inv_size * 0.5;
                        corner.x += player_inv_size.x * 0.5;
                        corner.x += padding * 0.5;

                        let rows = PLAYER_HOTBAR_SIZE;
                        let cols = PLAYER_ROW_SIZE;

                        let size = Vec2::new(rows as f32, cols as f32) * (slot_size + padding) as f32;

                        renderer.draw_rect(corner, size, Vec4::ONE);

                        corner += padding;
                        renderer.draw_rect(corner, Vec2::splat(slot_size), Vec4::ZERO.with_w(1.0));


                        let work_slot = game.structures.work_queue.find(*structure_id);

                        let structure = game.structures.get_mut(*structure_id);
                        let inventory = structure.inventory.as_mut().unwrap();

                        let mouse = renderer.to_point(input.mouse_position());
                        let input_kind = {
                            let (input, _) = inventory.input(0);
                            input.map(|x| x.kind)
                        };

                        draw_inventory_item(
                            renderer,
                            &mut inventory.slots,
                            game.player.body.position,
                            &mut game.entities,
                            &mut Some(&mut game.player.inventory),
                            input,
                            holding_item,
                            Vec2::new(corner.x, corner.y),
                            0,
                            COLOUR_GREY,
                            |kind| FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == kind).is_some()
                        );




                        draw_inventory_item(
                            renderer,
                            &mut inventory.slots,
                            game.player.body.position,
                            &mut game.entities,
                            &mut Some(&mut game.player.inventory),
                            input,
                            holding_item,
                            Vec2::new(corner.x, corner.y+padding+slot_size),
                            1,
                            COLOUR_GREY,
                            |kind| Filter::Fuel.is_valid(kind),
                        );


                        corner.x += padding;
                        corner.x += slot_size;

                        let mut bar_size = size - slot_size * 2.0 - padding * 4.0;
                        bar_size.y = slot_size - padding * 2.0;
                        renderer.draw_rect(
                            Vec2::new(corner.x, corner.y+padding),
                            bar_size,
                            COLOUR_DARK_GREY,
                        );




                        let StructureData::Furnace(furnace) = &structure.data
                        else { unreachable!() };

                        // smelt bar
                        'block: {
                        if let Some(tick) = work_slot {
                            // figure out the recipe time
                            let (Some(item), _)= inventory.input(0)
                            else { break 'block };


                            let Some(recipe) = FURNACE_RECIPES.iter()
                                .find(|x| x.requirements[0].kind == item.kind)
                            else { unreachable!() };


                            let time = recipe.time * furnace.multiplier;
                            let start_time = tick.u32() - time;
                            let diff = game.current_tick.u32() - start_time;

                            let progress = (diff as f64 / time as f64) as f32;


                            renderer.draw_rect(
                                Vec2::new(corner.x, corner.y+padding),
                                Vec2::new(bar_size.x * progress, bar_size.y),
                                Vec4::new(0.0, 1.0, 0.0, 1.0),
                            );


                        }
                        }


                        // fuel bar
                        {
                            let curr = structure.energy.energy;
                            let max = COAL_ENERGY_PER_UNIT;
                            let progress = (curr as f64 / max as f64) as f32;


                            renderer.draw_rect(
                                Vec2::new(corner.x, corner.y+padding+slot_size+padding),
                                Vec2::new(bar_size.x, bar_size.y),
                                COLOUR_GREY,
                            );

                            renderer.draw_rect(
                                Vec2::new(corner.x, corner.y+padding+slot_size+padding),
                                Vec2::new(bar_size.x * progress, bar_size.y),
                                Vec4::new(1.0, 0.0, 0.0, 1.0),
                            );
                        }

                        corner.x += bar_size.x;
                        corner.x += padding;


                        draw_inventory_item(
                            renderer,
                            &mut inventory.slots,
                            game.player.body.position,
                            &mut game.entities,
                            &mut Some(&mut game.player.inventory),
                            input,
                            holding_item,
                            Vec2::new(corner.x, corner.y),
                            2,
                            COLOUR_GREY,
                            |_| false,
                        );

                        if let Some(tick) = work_slot {
                            let (input, _) = inventory.input(0);
                            let (output, _) = inventory.output(0);


                            let should_cancel = match (input, output) {
                                (None, _) => true,
                                (Some(input), None) => {
                                    let Some(recipe) = FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == input.kind)
                                    else { unreachable!() };


                                       recipe.requirements[0].amount > input.amount
                                    || (if let Some(ik) = input_kind { ik != input.kind } else { false })

                                },
                                (Some(input), Some(output)) => {
                                    let Some(recipe) = FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == input.kind)
                                    else { unreachable!() };


                                       recipe.requirements[0].amount > input.amount
                                    || (if let Some(ik) = input_kind { ik != input.kind } else { false })
                                    || recipe.result.kind != output.kind
                                    || recipe.result.amount + output.amount > output.kind.max_stack_size()
                                },
                            };


                            if should_cancel {
                                structure.is_asleep = true;
                                game.structures.work_queue.remove(tick, *structure_id);
                            }

                        }

                    }




                    InventoryMode::Assembler(structure) => {
                        let mut corner = window * 0.5 - player_inv_size * 0.5;
                        corner.x += player_inv_size.x * 0.5;
                        corner.x += padding * 0.5;

                        let rows = PLAYER_HOTBAR_SIZE;
                        let cols = PLAYER_ROW_SIZE;

                        let size = Vec2::new(rows as f32, cols as f32) * (slot_size + padding) as f32;

                        renderer.draw_rect(corner, size, Vec4::ONE);


                        let text = "\n  \
                              §8Left Click §8to §2set §8recipe  \n\n\
                        ";

                        let text_size = renderer.text_size(text, 0.6);

                        let text_pos = window * 0.5 - Vec2::new(text_size.x * 0.5, text_size.y + player_inv_size.y * 0.5 + padding);
                        renderer.draw_rect(text_pos, text_size, Vec4::ONE);
                        renderer.draw_text(text, text_pos, 0.6, Vec4::new(0.2, 0.2, 0.2, 1.0));
                        renderer.draw_rect(corner, size, Vec4::ONE);



                        let mut base = corner + padding * 0.5;
                        let point = renderer.to_point(input.mouse_position());
                        for col in 0..cols {
                            let mut pos = base;
                            for row in 0..rows {
                                let recipe_index = col*rows+row;
                                let Some(&curr_recipe) = RECIPES.get(recipe_index)
                                else { break 'mode };


                                if curr_recipe.result.kind == ItemKind::Radar { continue }

                                let mut close = false;
                                draw_inventory_slot(
                                    &mut (),
                                    renderer,
                                    input,
                                    pos,
                                    Some(curr_recipe.result),
                                    COLOUR_GREY,
                                    |renderer, _| {
                                        default_hover_action(renderer, point, curr_recipe.result.kind);
                                    },

                                    |_, _| {
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
                                                game.entities.spawn(
                                                    EntityKind::dropped_item(item),
                                                    game.player.body.position);
                                            }
                                        }

                                        close = true;

                                    },
                                    |_, _| {},
                                    |_, _| {},
                                );


                                if close {
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

                        draw_recipes(game, input, renderer, holding_item, corner);
                    },
                }
                }

                let mut corner = window * 0.5 - player_inv_size * 0.5;
                corner.x -= player_inv_size.x * 0.5;
                corner.x -= padding * 0.5;

                draw_player_inventory(renderer, &mut game.player, &mut game.world, &mut game.entities, &mut other_inv, input, holding_item, corner);
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
                    let _ = writeln!(text, "§eSAVE TIME ELAPSED: §a{:.1}§r", game.current_tick.u32() as f64 / TICKS_PER_SECOND as f64);
                    let _ = writeln!(text, "§eRENDER DISTANCE: §a{}§r", game.settings.render_distance);
                    let _ = writeln!(text, "§eTRIANGLE COUNT: §a{}§r", renderer.triangle_count.get());
                    renderer.triangle_count.set(0);
                    renderer.draw_count.set(0);

                    let _ = writeln!(text, "§eRENDER WORLD TIME: §a{}ms§r", game.render_world_time);
                    let _ = writeln!(text, "§eRENDERED CHUNKS: §a{}§r", game.total_rendered_chunks);
                    let _ = writeln!(text, "§eCHUNK LOAD QUEUE: §a{}§r", game.world.chunker.chunk_load_queue_len());
                    let _ = writeln!(text, "§eCHUNK ACTIVE JOBS: §a{}§r", game.world.chunker.chunk_active_jobs_len());
                    let _ = writeln!(text, "§eREMESH QUEUE: §a{}§r", game.world.chunker.mesh_load_queue_len());
                    let _ = writeln!(text, "§eREMESH ACTIVE JOBS: §a{}§r", game.world.chunker.mesh_active_jobs_len());
                    let _ = writeln!(text, "§eMESH UNLOAD QUEUE JOBS: §a{}§r", game.world.chunker.mesh_unload_queue_len());

                    let _ = writeln!(text, "§ePITCH: §a{:.1}({:.1}) §eYAW: §a{:.1}({:.1})§r", game.camera.pitch.to_degrees(), game.camera.pitch, game.camera.yaw.to_degrees(), game.camera.yaw);
                    let _ = writeln!(text, "§ePOSITION: §a{:.1}, {:.1} {:.1}§r", game.camera.position.x, game.camera.position.y, game.camera.position.z);

                    let (chunk_pos, chunk_local_pos) = split_world_pos(game.player.body.position.floor().as_ivec3());
                    let _ = writeln!(text, "§eCHUNK POSITION: §a{}, {}, {}§r", chunk_pos.0.x, chunk_pos.0.y, chunk_pos.0.z);
                    let _ = writeln!(text, "§eCHUNK LOCAL POSITION: §a{}, {}, {}§r", chunk_local_pos.x, chunk_local_pos.y, chunk_local_pos.z);
                    let _ = writeln!(text, "§eCHUNK VERSION: §a{}§r", game.world.chunker.get_chunk(chunk_pos).map(|x| x.version.get()).unwrap_or(0));
                    match game.world.chunker.get_mesh_entry(chunk_pos) {
                        MeshEntry::None => {
                            let _ = writeln!(text, "§eMESH VERSION: §aNone§r");
                        },
                        MeshEntry::Loaded(chunk_meshes) => {
                            let _ = writeln!(text, "§eMESH VERSION: §a{}§r", chunk_meshes.version.get());
                        },
                    };

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
                            let _ = writeln!(text, "§e- ENERGY: §b{}", structure.energy.energy);

                            if let Some(inv) = &structure.inventory {
                                let input_len = inv.inputs_len();
                                if input_len > 0 {
                                    let _ = writeln!(text, "§e  - INPUTS:");
                                    for i in 0..input_len {
                                        let (item, meta) = inv.input(i);
                                        let filter = match meta.kind {
                                            SlotKind::Input { filter } => filter,
                                            SlotKind::Storage => Filter::None,
                                            SlotKind::Output => unreachable!(),
                                        };

                                        if let Some(item) = item {
                                            let max_amount = meta.max_amount.min(item.kind.max_stack_size());
                                            let _ = writeln!(text, "§e     - §b{:?} §a{}x/{}x", item.kind, item.amount, max_amount);
                                        } else if !matches!(filter, Filter::None) && meta.max_amount != u32::MAX {
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

                                StructureData::Furnace(_) => {
                                    let _ = writeln!(text, "Furnace");
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
                    


                    if game.entities.entities.len() != 0 {
                        let _ = writeln!(text, "§eENTITIES:");

                        let mut i = 0;
                        for (_, entity) in game.entities.entities.iter() {
                            let _ = writeln!(text, "§e- §b{:?}§e: §a{:.1}, {:.1}, {:.1}", entity.kind, entity.body.position.x, entity.body.position.y, entity.body.position.z);
                            i += 1;
                            if i > 3 && i < game.entities.entities.len() {
                                let len = game.entities.entities.len();
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


            UILayer::Credits { time, audio } => {
                *time += dt;
                let window = renderer.window_size();
                renderer.draw_rect(Vec2::ZERO, window, Vec4::new(0.0, 0.0, 0.0, 1.0));

                let line_time = 1.5f32;
                let text = r#"
                §7...
                §7*You suddenly hear a beep!*
                §7Initializing radar link...
                §7Attempting to connect to §eEarth-616
                §7Signal Strength: §444%

                §eEarth-616: §fWe got a connection!
                §eEarth-616: §fHello... anyone there?


                §eEarth-616: §fUhh?
                §eEarth-616: §fAre we on the wrong frequency...

                §eEarth-616: §fI guess.
                

                
                §4Connection was terminated.
                "#;

                let credits = r#"
                §aTHANKS FOR PLAYING!

                §7DIRECTOR:§e Me
                §7PRODUCER:§e Me
                §7EXECUTIVE PRODUCER:§e Me
                §7LEAD DESIGNER:§e Me
                §7GAMEPLAY DESIGNER:§e Me
                §7LEVEL DESIGNER:§e Me
                §7NARRATIVE DESIGNER:§e Me
                §7STORY WRITER:§e Me
                §7SCRIPT EDITOR:§e Me
                §7CUTSCENE DIRECTOR:§e Me
                §7CINEMATOGRAPHER:§e this doesn't even make sense
                §7EDITOR:§e Me
                §7UI/UX DESIGNER:§e yeah.. sorry
                §7CONCEPT ARTIST:§e Me
                §7CHARACTER ARTIST:§e Me
                §7ENVIRONMENT ARTIST:§e Me
                §7PROP DESIGNER:§e Me
                §7TEXTURE ARTIST:§e Me
                §7ANIMATOR:§e Me
                §7RIGGER:§e Me
                §7VISUAL EFFECTS:§e Me
                §7LIGHTING ARTIST:§e Me
                §7SHADER ARTIST:§e Me
                §7SOUND DESIGNER:§e what sound?
                §7SOUND EDITOR:§e Me
                §7COMPOSER:§e Me
                §7VOICE DIRECTOR:§e Me
                §7VOICE ACTING:§e [awkward silence]
                §7AI PROGRAMMER:§e Me
                §7GAMEPLAY PROGRAMMER:§e Me
                §7NETWORK ENGINEER:§e Me
                §7TOOLS PROGRAMMER:§e Me
                §7PHYSICS ENGINEER:§e Me
                §7BUILD ENGINEER:§e Me
                §7QA LEAD:§e Me
                §7BUG HUNTER:§e there were bugs?
                §7QA AUTOMATION:§e Me
                §7PLAYTESTER:§e Me
                §7ACCESSIBILITY LEAD:§e Me
                §7LOCALIZATION LEAD:§e Me
                §7TRANSLATOR:§e Me
                §7MARKETING DIRECTOR:§e Me
                §7COMMUNITY MANAGER:§e Me
                §7SOCIAL MEDIA INTERN:§e Me
                §7BRAND STRATEGIST:§e Me
                §7TECHNICAL SUPPORT:§e Me
                §7IT DEPARTMENT:§e Me
                §7LEGAL TEAM:§e Me
                §7FINANCE:§e Me
                §7HR MANAGER:§e it's just me bro
                §7JANITOR:§e Me
                §7COFFEE SUPPLIER:§e Me
                §7MORALE OFFICER:§e pizza
                §7MEME CURATOR:§e Me
                §7FUN POLICE:§e also me
                §7SPECIAL THANKS:§e leddoo, Kiniro
                §7EXTRA NOTE:§e I worked at Blizzard for 6 years btw
                "#;


                let text_size = renderer.text_size(text, 1.0);
                let pos = (window - text_size) * 0.5;
                let max_time = text.lines().count() as f32 * line_time + 0.5;


                let final_str_len = text.bytes()
                    .enumerate()
                    .filter(|x| x.1 == b'\n')
                    .skip((*time / line_time).floor() as usize)
                    .next()
                    .map(|x| x.0)
                    .unwrap_or(text.len());

                let final_text = &text[..final_str_len];

                let colour = max_time - *time;
                println!("{colour} {max_time}");
                renderer.draw_text(final_text, pos, 1.0, Vec4::ONE.with_w(colour));


                let time = *time - max_time - 2.5;
                if time < 0.0 {
                    return;
                }

                match audio.state() {
                    kira::sound::PlaybackState::Paused => {
                        audio.resume(Tween::default());
                    },
                    _ => (),
                }

                let line_height = renderer.line_size;
                let mut y = window.y - time * 48.0; 

                for (i, line) in credits.lines().enumerate() {
                    let scale = if i == 1 { 3.0 } else { 1.0 };
                    let line = line.trim();
                    let text_size = renderer.text_size(line, scale);
                    renderer.draw_text(
                        line,
                        Vec2::new((window.x - text_size.x) * 0.5, y),
                        scale,
                        Vec4::ONE,
                    );

                    if line.is_empty() {
                        y += line_height;
                    }
                    y += text_size.y * 1.2;
                }


                if y < -line_height {
                    self.close(game, dt);
                }
                


            }


            UILayer::None => unreachable!(),
        }
    }
}



fn draw_recipes(game: &mut Game, input: &InputManager, renderer: &mut Renderer, _: &mut Option<Item>, corner: Vec2) {
    let rows = PLAYER_HOTBAR_SIZE;
    let cols = PLAYER_ROW_SIZE;

    let slot_size = 64.0;
    let padding = 16.0;

    let size = Vec2::new(rows as f32, cols as f32) * (slot_size + padding) as f32;

    renderer.draw_rect(corner, size, COLOUR_WHITE);

    let mut base = corner + padding * 0.5;
    let point = renderer.to_point(input.mouse_position());
    for col in 0..cols {
        let mut pos = base;
        for row in 0..rows {
            // render
            let Some(&recipe) = RECIPES.get(col*rows+row)
            else { return };

            let (can_craft, mut rc) = RecipeCraft::try_craft(game.player.inventory, recipe);
            let is_mouse_intersecting = point_in_rect(point, pos, Vec2::splat(slot_size));

            if is_mouse_intersecting && can_craft && input.is_button_just_pressed(MouseButton::Left) {
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

            let mut colour = if can_craft { COLOUR_PASS }
                             else { COLOUR_DENY }; 

            if is_mouse_intersecting {
                colour += COLOUR_ADDITIVE_HIGHLIGHT;
            }
           
            renderer.draw_rect(pos, Vec2::splat(slot_size), colour);
            renderer.draw_item_icon(recipe.result.kind, pos+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);
            renderer.draw_text(format!("{}", recipe.result.amount).as_str(), pos+slot_size*0.05, 0.5, Vec4::ONE);


            if is_mouse_intersecting {
                let padding = 10.0;
                let scale = 0.5;

                let text_size = renderer.text_size(recipe.result.kind.name(), scale);
                let ingredient_size = Vec2::new(recipe.requirements.len() as f32, 1.0) * (padding*0.5 + slot_size);

                let time_text = format!("§e{}s §fCrafting time", recipe.time as f32 / TICKS_PER_SECOND as f32);
                let time_text_size = renderer.text_size(&time_text, scale);

                let mut str = String::new();
                let max_text_size = {
                    let mut size = text_size.x.max(time_text_size.x);
                    for item in recipe.requirements.iter() {
                        str.clear();
                        write!(str, "{}x {}", item.amount, item.kind.name()).unwrap();
                        let text_size = renderer.text_size(&str, scale);
                        size = size.max((text_size + padding + slot_size).x);
                    }

                    size
                };

                let size = Vec2::splat(padding * 0.5)
                            + Vec2::new(max_text_size, ingredient_size.x)
                            + Vec2::new(0.0, text_size.y)
                            + Vec2::new(0.0, padding)
                            + Vec2::new(0.0, time_text_size.y)
                            + Vec2::splat(padding * 2.0);


                renderer.with_z(UI_Z_MAX, |renderer| {
                let mut pos = point + UI_HOVER_ACTION_OFFSET;
                pos.y -= size.y * 0.5;

                renderer.draw_rect(pos, size, COLOUR_DARK_GREY);
                renderer.draw_text(recipe.result.kind.name(), pos+padding, scale, Vec4::ONE);

                let mut base = pos + padding + Vec2::new(0.0, text_size.y+padding);
                for item in recipe.requirements.iter() {
                    let craft_step = rc.craft_queue.iter()
                        .find(|x| x.item == item.kind && x.depth == 1)
                        .map(|x| x.result)
                        .unwrap();

                    let colour = match craft_step {
                        CraftStepResult::DirectlyAvailable => COLOUR_PASS,
                        CraftStepResult::Craftable(_) => COLOUR_WARN,
                        CraftStepResult::NotCraftable => COLOUR_DENY,
                        CraftStepResult::NotAvailableRawMaterial => COLOUR_DENY,
                    };

                    renderer.draw_rect(base, Vec2::splat(slot_size), colour);
                    renderer.draw_item_icon(item.kind, base+slot_size*0.05, Vec2::splat(slot_size*0.9), Vec4::ONE);

                    str.clear();
                    write!(str, "{}x {}", item.amount, item.kind.name()).unwrap();
                    let text_size = renderer.text_size(&str, scale);

                    renderer.draw_text_ex(&str, base + Vec2::new(padding + slot_size, (slot_size - text_size.y) * 0.5), scale, colour, true);
                    base += Vec2::new(0.0, slot_size+padding*0.5);
                }

                base.y += padding * 0.5;
                renderer.draw_text(&time_text, base, scale, Vec4::ONE);
                base.y += padding * 0.5;
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


fn draw_player_inventory(renderer: &mut Renderer, player: &mut Player, world: &mut VoxelWorld, entities: &mut EntityMap, other_inv: &mut Option<&mut [Option<Item>]>, input: &InputManager, holding_item: &mut Option<Item>, corner: Vec2) {
    let rows = PLAYER_ROW_SIZE;
    let cols = PLAYER_HOTBAR_SIZE;

    let slot_size = 64.0;
    let padding = 16.0f32;

    let size = Vec2::new(cols as f32, rows as f32) * (slot_size + padding) as f32;

    renderer.draw_rect(corner, size, COLOUR_WHITE);

    let mut base = corner + padding * 0.5;
    let point = renderer.to_point(input.mouse_position());
    for row in 0..rows {
        let mut pos = base;
        for col in 0..cols {
            let slot_index = row*cols+col;
            let is_mouse_intersecting = point_in_rect(point, pos, Vec2::splat(SLOT_SIZE));
            let colour = if slot_index/PLAYER_HOTBAR_SIZE == player.hotbar { COLOUR_PLAYER_ACTIVE_HOTBAR }
                         else { COLOUR_GREY }; 


            draw_inventory_item(renderer, &mut player.inventory, player.body.position, entities, other_inv, input, holding_item,
                                pos, slot_index, colour, |_| true);

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
                  player_pos: DVec3, world: &mut VoxelWorld, entities: &mut EntityMap,
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
            let colour = COLOUR_GREY; 

            draw_inventory_item(renderer, inventory, player_pos, entities, &mut other_inv, input, holding_item,
                                pos, slot_index, colour, |_| true);

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



fn draw_inventory_slot<T>(
    data: &mut T,
    renderer: &mut Renderer, input: &InputManager, at: Vec2, item: Option<Item>,
    mut colour: Vec4,
    hover_action: impl FnOnce(&mut Renderer, &mut T),
    left_click_action: impl FnOnce(&mut Renderer, &mut T),
    right_click_action: impl FnOnce(&mut Renderer, &mut T),
    oth: impl FnOnce(&mut Renderer, &mut T),
) {
    let mouse_pos = renderer.to_point(input.mouse_position());

    let is_mouse_intersecting = point_in_rect(mouse_pos, at, Vec2::splat(SLOT_SIZE));

    if is_mouse_intersecting {
        colour += COLOUR_ADDITIVE_HIGHLIGHT;
    }

    renderer.draw_rect(at, Vec2::splat(SLOT_SIZE), colour);

    if let Some(item) = item {
        renderer.draw_item_icon(item.kind, at+SLOT_SIZE*0.05, Vec2::splat(SLOT_SIZE*0.9), Vec4::ONE);
        if item.amount > 1 {
            renderer.draw_text(format!("{}", item.amount).as_str(), at+SLOT_SIZE*0.05, 0.5, Vec4::ONE);
        }
    }


    if !is_mouse_intersecting {
        return;
    }


    renderer.with_z(UI_Z_MAX, |renderer| {
        hover_action(renderer, data);
    });

    if input.is_button_just_pressed(MouseButton::Left) {
        left_click_action(renderer, data);
    }
    else if input.is_button_just_pressed(MouseButton::Right) {
        right_click_action(renderer, data);
    }

    oth(renderer, data);


}


fn default_hover_action(renderer: &mut Renderer, mouse_pos: Vec2, item: ItemKind) {
    let item_name = item.name();
    let scale = 0.5;
    let padding = 10.0;
    let size = renderer.text_size(item_name, scale) + Vec2::splat(padding * 2.0);

    let mut pos = mouse_pos + UI_HOVER_ACTION_OFFSET;
    pos.y -= size.y * 0.5;

    renderer.draw_rect(pos, size, COLOUR_DARK_GREY);
    renderer.draw_text(item_name, pos+padding, scale, Vec4::ONE);
}



fn draw_inventory_item(renderer: &mut Renderer, inventory: &mut [Option<Item>],
                       player_pos: DVec3, entities: &mut EntityMap,
                       other_inv: &mut Option<&mut [Option<Item>]>,
                       input: &InputManager, holding_item: &mut Option<Item>,
                       pos: Vec2, index: usize, colour: Vec4, filter: impl FnOnce(ItemKind) -> bool) {

    let mouse_pos = renderer.to_point(input.mouse_position());
    let item_slot = &mut inventory[index];
    let item = *item_slot;
    draw_inventory_slot(
        &mut (item_slot, holding_item),
        renderer, input, pos, item, colour,
        |renderer, (item_slot, _)| {
            let Some(item) = item_slot
            else { return; };
            default_hover_action(renderer, mouse_pos, item.kind);
        }, 


        |_, (item_slot, holding_item)| {
            if input.is_key_pressed(KeyCode::ShiftLeft)
                && let Some(other_inv) = other_inv 
                && let Some(inv_item) = item_slot {
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

                    **item_slot = None;
                    return;
                }


                for slot in other_inv.iter_mut() {
                    if slot.is_some() { continue }

                    if inv_item.amount != 0 {
                        *slot = Some(*inv_item);
                    }

                    **item_slot = None;
                    return ;
                }
            } else {
                if let Some(item) = holding_item && !filter(item.kind) {
                    return;
                }
                if let Some(inv_item) = item_slot
                   && let Some(item) = holding_item
                   && inv_item.kind == item.kind {

                    let addition = item.amount.min(inv_item.kind.max_stack_size().max(inv_item.amount) - inv_item.amount);

                    inv_item.amount += addition;

                    item.amount -= addition;
                    if item.amount == 0 {
                        **holding_item = None;
                    }
                    return;
                }

                let item = **item_slot;
                **item_slot = **holding_item;
                **holding_item = item;
                return;
            }

        },


        |_, (item_slot, holding_item)| {
            if let Some(item) = item_slot && holding_item.is_none() {
                let amount = item.amount;
                item.amount -= amount / 2;

                let mut new_item = *item;
                new_item.amount = amount / 2;
                if new_item.amount != 0 {
                    **holding_item = Some(new_item);
                    return;
                }
            }

            let item = **item_slot;
            **item_slot = **holding_item;
            **holding_item = item;
        },


        |_, (item_slot, _)| {
            if input.is_key_just_pressed(KeyCode::KeyQ)
                && let Some(item) = item_slot {

                let mut drop_item = *item;
                if input.is_alt_pressed() {
                    drop_item.amount = 1;
                }

                item.amount -= drop_item.amount;
                let item = *item;
                if item.amount == 0 {
                    **item_slot = None;
                }

                entities.spawn(EntityKind::dropped_item(item), player_pos);
            }

        }

    );
}
