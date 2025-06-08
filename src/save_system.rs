use std::{collections::HashMap, fmt::Write, ops::Bound};

use glam::{Vec2, Vec3};
use rand::seq::IndexedRandom;
use save_format::{Arena, Value};
use sti::format_in;

use crate::{crafting::RECIPES, directions::CardinalDirection, items::{DroppedItem, Item, ItemKind}, structures::{strct::{InserterState, Structure, StructureData, StructureKind}, Crafter}, Game, PhysicsBody, Tick, DROPPED_ITEM_SCALE};

impl Game {
    #[allow(unused_must_use)]
    pub fn load(&mut self) {
        let mut game = Game::new();

        let Ok(file) = std::fs::read_to_string("saves/world.sft")
        else { return };
        let arena = save_format::Arena::new();

        let hm = save_format::parse_str(&arena, &file).unwrap();

        game.current_tick = Tick(hm["current_tick"].as_u32());
        game.structures.current_tick = game.current_tick;

        game.ui_scale = hm["ui_scale"].as_f32();

        game.camera.yaw = hm["camera.yaw"].as_f32();
        game.camera.pitch = hm["camera.pitch"].as_f32();

        let mut i = 0;
        let mut buf = sti::string::String::with_cap_in(&arena, 128);
        loop {
            buf.clear();
            write!(buf, "world.dropped_items[{i}]");
            println!("{i}");

            let Some(dropped_item) = parse_dropped_item(&hm, &mut buf)
            else { break; };

            game.world.dropped_items.push(dropped_item);

            i += 1;
        }


        // player
        game.player.body.position = hm["player.body.position"].as_vec3().as_dvec3();
        game.player.body.velocity = hm["player.body.velocity"].as_vec3();
        game.player.hand = hm["player.hand"].as_u32() as usize;

        let mut i = 0;
        loop {
            if i >= game.player.inventory.len() { break };

            buf.clear();
            write!(buf, "player.inventory[{i}]");

            let Some(&value) = hm.get(buf.as_str())
            else { i += 1; continue };

            let item = parse_item(value.as_str());
            game.player.inventory[i] = Some(item);

            i += 1;
        }


        let mut i = 0;
        let mut buf = sti::string::String::with_cap_in(&arena, 128);
        loop {
            buf.clear();
            write!(buf, "player.pulling[{i}]");

            let Some(dropped_item) = parse_dropped_item(&hm, &mut buf)
            else { break; };

            game.player.pulling.push(dropped_item);

            i += 1;
        }


        // structures!
        // yippie, my favourite

        let mut i = 0;
        let mut buf = sti::string::String::with_cap_in(&arena, 128);
        loop {
            buf.clear();
            write!(buf, "structure[{i}].kind");
            let Some(kind) = hm.get(buf.as_str())
            else { break };

            let item_kind = *ItemKind::ALL.iter().find(|f| f.to_string() == kind.as_str()).unwrap();
            let ItemKind::Structure(kind) = item_kind
            else { unreachable!() };

            buf.clear();
            write!(buf, "structure[{i}].origin");
            let origin = hm[buf.as_str()].as_vec3().as_ivec3();
            buf.clear();
            write!(buf, "structure[{i}].direction");
            let direction = match hm[buf.as_str()].as_str() {
                "north" => CardinalDirection::North,
                "south" => CardinalDirection::South,
                "east" => CardinalDirection::East,
                "west" => CardinalDirection::West,
                _ => unreachable!(),
            };

            let data = match kind {
                StructureKind::Quarry => {
                    buf.clear();
                    write!(buf, "structure[{i}].current_progress");
                    let current_progress = hm[buf.as_str()].as_u32();

                    buf.clear();
                    write!(buf, "structure[{i}].output");
                    let output = hm.get(buf.as_str()).map(|str| parse_item(str.as_str()));

                    StructureData::Quarry { current_progress, output }
                },


                StructureKind::Inserter => {
                    buf.clear();
                    write!(buf, "structure[{i}].filter");
                    let filter = hm.get(buf.as_str()).map(|str| ItemKind::ALL.iter().find(|f| f.to_string() == str.as_str()).unwrap()).copied();

                    buf.clear();
                    write!(buf, "structure[{i}].state");
                    let state = match hm[buf.as_str()].as_str() {
                        "searching" => InserterState::Searching,
                        "placing" => {
                            buf.clear();
                            write!(buf, "structure[{i}].item");
                            let item = parse_item(hm[buf.as_str()].as_str());

                            InserterState::Placing(item)
                        }

                        _ => unreachable!(),
                    };

                    StructureData::Inserter { state, filter }
                },


                StructureKind::Chest => {
                    let mut inv_i = 0;
                    let mut inventory = [None; 3*3];
                    while inv_i < inventory.len() {
                        buf.clear();
                        write!(buf, "structure[{i}].inventory[{inv_i}]");
                        let Some(str) = hm.get(buf.as_str())
                        else { inv_i += 1; continue; };

                        let item = parse_item(str.as_str());
                        inventory[inv_i] = Some(item);
                        inv_i += 1;
                    }

                    StructureData::Chest { inventory }
                },


                StructureKind::Silo => {
                    let mut inv_i = 0;
                    let mut inventory = [None; 6*6];
                    while inv_i < inventory.len() {
                        buf.clear();
                        write!(buf, "structure[{i}].inventory[{inv_i}]");
                        let Some(str) = hm.get(buf.as_str())
                        else { inv_i += 1; continue; };

                        let item = parse_item(str.as_str());
                        inventory[inv_i] = Some(item);
                        inv_i += 1;
                    }

                    StructureData::Silo { inventory }
                },


                StructureKind::Belt => {
                    let mut inv_i = 0;
                    let mut inventory = [[None; 2]; 2];
                    while inv_i < 4 {
                        buf.clear();
                        write!(buf, "structure[{i}].inventory[{inv_i}]");
                        let Some(str) = hm.get(buf.as_str())
                        else { inv_i += 1; continue; };

                        let item = parse_item(str.as_str());
                        inventory[inv_i/2][inv_i%2] = Some(item);
                        println!("lane: {}, item: {}", inv_i/2, inv_i%2);
                        inv_i += 1;

                    }


                    StructureData::Belt { inventory }
                },


                StructureKind::Splitter => {
                    let mut inv_i = 0;
                    let mut inventory = [[[None; 2]; 2]; 2];
                    while inv_i < 8 {
                        buf.clear();
                        write!(buf, "structure[{i}].inventory[{inv_i}]");
                        let Some(str) = hm.get(buf.as_str())
                        else { inv_i += 1; continue; };

                        let item = parse_item(str.as_str());
                        let x = inv_i/4;
                        let y = (inv_i%4)/2;
                        let z = inv_i%2;
                        inventory[x][y][z] = Some(item);
                        inv_i += 1;
                    }


                    let priority = [
                        {
                            buf.clear();
                            write!(buf, "structure[{i}].priority[0]");
                            hm[&*buf].as_u32() as u8
                        },
                        {
                            buf.clear();
                            write!(buf, "structure[{i}].priority[1]");
                            hm[&*buf].as_u32() as u8
                        }

                    ];


                    StructureData::Splitter { inventory, priority }

                },


                StructureKind::Assembler => {
                    buf.clear();
                    write!(buf, "structure[{i}].recipe");
                    let recipe = hm[&*buf].as_u32();
                    let recipe = RECIPES[recipe as usize];
                    let mut crafter = Crafter::from_recipe(recipe);

                    for (index, item) in crafter.inventory.iter_mut().enumerate() {
                        buf.clear();
                        write!(buf, "structure[{i}].inventory[{index}]");
                        dbg!(&buf);
                        let str = hm[&*buf].as_str();
                        let parsed_item = parse_item(&str);

                        if parsed_item.kind == item.kind {
                            item.amount += parsed_item.amount;
                        } else {
                            game.world.dropped_items.push(DroppedItem::new(*item, origin.as_dvec3()));
                        }
                    }


                    buf.clear();
                    write!(buf, "structure[{i}].output");
                    let str = hm[&*buf].as_str();
                    let output = parse_item(&str);
                    if output.kind == crafter.output.kind {
                        crafter.output.amount += output.amount;
                    } else {
                        game.world.dropped_items.push(DroppedItem::new(output, origin.as_dvec3()));
                    }

                    StructureData::Assembler { crafter }
                }



                StructureKind::Furnace => {
                    StructureData::from_kind(StructureKind::Furnace)
                }
            };


            let structure = Structure {
                position: origin,
                direction,
                data,
                is_asleep: true,
            };

            game.structures.add_structure(&mut game.world, structure);
            i += 1;
        }

        *self = game;
    }


    pub fn save(&mut self) {
        let mut v = Vec::new();

        macro_rules! insert {
            ($k: expr, $ty: ident) => {
                v.push((&stringify!($k)[5..], Value::$ty($k as _)))
                
            };
        }

        self.world.save();

        let arena = Arena::new();
        v.push(("current_tick", Value::Num(self.current_tick.u32() as f64)));
        v.push(("ui_scale", Value::Num(self.ui_scale as f64)));

        insert!(self.camera.yaw, Num);
        insert!(self.camera.pitch, Num);


        for (i, item) in self.world.dropped_items.iter().enumerate() {
            let path = format_in!(&arena, "world.dropped_items[{i}]").leak();
            save_dropped_item(&arena, &mut v, path, item);
        }


        v.push(("player.body.position", Value::Vec3(self.player.body.position.as_vec3())));
        insert!(self.player.body.velocity, Vec3);
        insert!(self.player.hand, Num);

        
        for (i, item) in self.player.inventory.iter().enumerate() {
            let path = format_in!(&arena, "player.inventory[{i}]").leak();
            if let Some(item) = item {
                save_item(&arena, &mut v, path, *item);
            }
        }


        for (i, item) in self.player.pulling.iter().enumerate() {
            let path = format_in!(&arena, "player.pulling[{i}]").leak();
            save_dropped_item(&arena, &mut v, path, item);
        }


        // structures
        let mut buf = String::new();
        let mut structure_to_index = HashMap::new();
        let mut i = 0;
        for (id, structure) in self.structures.structs.iter() {
            buf.clear();
            let _ = write!(buf, "structure[{i}]");
            structure_to_index.insert(id, i);

            i += 1;
            v.push((format_in!(&arena, "{buf}.kind").leak(), Value::String(structure.data.as_kind().item_kind().to_string())));
            v.push((format_in!(&arena, "{buf}.origin").leak(), Value::Vec3(structure.position.as_vec3())));

            let direction = match structure.direction {
                CardinalDirection::North => "north",
                CardinalDirection::South => "south",
                CardinalDirection::East => "east",
                CardinalDirection::West => "west",
            };

            v.push((format_in!(&arena, "{buf}.direction").leak(), Value::String(direction)));

            match &structure.data {
                StructureData::Quarry { current_progress, output } => {
                    v.push((format_in!(&arena, "{buf}.current_progress").leak(), Value::Num(*current_progress as f64)));
                    if let Some(output) = *output {
                        let path = format_in!(&arena, "{buf}.output").leak();
                        save_item(&arena, &mut v, path, output);
                    }
                },


                StructureData::Inserter { state, filter } => {
                    if let Some(filter) = filter {
                        v.push((format_in!(&arena, "{buf}.filter").leak(), Value::String(filter.to_string())));
                    }


                    let state = match state {
                        InserterState::Searching => "searching",
                        InserterState::Placing(item) => {
                            let path = format_in!(&arena, "{buf}.item").leak();
                            save_item(&arena, &mut v, &path, *item);

                            "placing"
                        },
                    };

                    v.push((format_in!(&arena, "{buf}.state").leak(), Value::String(state)));
                },


                StructureData::Chest { inventory } => {
                    for (i, slot) in inventory.iter().enumerate() {

                        let Some(item) = slot
                        else { continue };

                        let path = format_in!(&arena, "{buf}.inventory[{i}]").leak();
                        save_item(&arena, &mut v, path, *item);
                    }
                },


                StructureData::Silo { inventory } => {
                    for (i, slot) in inventory.iter().enumerate() {

                        let Some(item) = slot
                        else { continue };

                        let path = format_in!(&arena, "{buf}.inventory[{i}]").leak();
                        save_item(&arena, &mut v, path, *item);
                    }
                },


                StructureData::Belt { inventory } => {
                    for (lane, items) in inventory.iter().enumerate() {

                        for (i, item) in items.iter().enumerate() {
                            let Some(item) = item
                            else { continue };

                            let path = format_in!(&arena, "{buf}.inventory[{}]", lane*2+i).leak();
                            save_item(&arena, &mut v, path, *item);

                        }
                    }
                },


                StructureData::Splitter { inventory, priority } => {
                    let inventory : [Option<Item>; 8] = unsafe { core::mem::transmute(*inventory) };

                    for (i, item) in inventory.iter().enumerate() {
                        let Some(item) = item
                        else { continue };

                        let path = format_in!(&arena, "{buf}.inventory[{}]", i).leak();
                        save_item(&arena, &mut v, path, *item);
                    }

                    v.push((format_in!(&arena, "{buf}.priority[0]").leak(), Value::Num(priority[0] as _)));
                    v.push((format_in!(&arena, "{buf}.priority[1]").leak(), Value::Num(priority[1] as _)));
                },


                StructureData::Assembler { crafter } => {
                    let (recipe_index, _) = RECIPES.iter().enumerate().find(|x| x.1 == &crafter.recipe).unwrap();

                    v.push((format_in!(&arena, "{buf}.recipe").leak(), Value::Num(recipe_index as _)));
                    let path = format_in!(&arena, "{buf}.output").leak();
                    save_item(&arena, &mut v, path, crafter.output);
                    for (i, item) in crafter.inventory.iter().enumerate() {
                        let path = format_in!(&arena, "{buf}.inventory[{i}]").leak();
                        save_item(&arena, &mut v, path, *item);
                    }
                }


                StructureData::Furnace { input, output } => {
                    if let Some(input) = input {
                        let path = format_in!(&arena, "{buf}.input").leak();
                        save_item(&arena, &mut v, path, *input);
                    }
                    if let Some(output) = output {
                        let path = format_in!(&arena, "{buf}.output").leak();
                        save_item(&arena, &mut v, path, *output);
                    }
                }
            };
        }


        // work queeu
        let mut cursor = self.structures.work_queue.entries.lower_bound(Bound::Unbounded);
        let mut i = 0;
        while let Some(((tick, id), ())) = cursor.next() {
            i += 1;
            let index = structure_to_index[&id.0];
            let lifetime = tick.u32() - self.current_tick.u32();
            v.push((format_in!(&arena, "work_queue[{i}]").leak(), Value::Vec2(Vec2::new(lifetime as f32, index as f32))));
        }

        // to be awoken
        let mut i = 0;
        for id in &self.structures.to_be_awoken {
            i += 1;
            let index = structure_to_index[&id.0];
            v.push((format_in!(&arena, "to_be_awoken[{i}]").leak(), Value::Num(index as f64)));
        }


        // craft queue
        if !self.craft_queue.is_empty() {
            println!("[warn] craft queue isn't saved currently");
        }

        std::fs::write("saves/world.sft", save_format::slice_to_string(&v)).unwrap();
    }



}



fn save_item<'a>(arena: &'a Arena,
                 v: &mut Vec<(&'a str, Value<'a>)>,
                 prefix: &'a str,
                 item: Item) {

    let output = format_in!(arena, "{} x{}", item.kind.to_string(), item.amount).leak();
    v.push((prefix, Value::String(output)));
}


fn save_dropped_item<'a>(
    arena: &'a Arena,
    v: &mut Vec<(&'a str, Value<'a>)>,
    prefix: &'a str,
    item: &DroppedItem) {
    let path = format_in!(arena, "{prefix}.item");
    save_item(arena, v, path.leak(), item.item);
    v.push((format_in!(arena, "{prefix}.body.position").leak(), Value::Vec3(item.body.position.as_vec3())));
    v.push((format_in!(arena, "{prefix}.body.velocity").leak(), Value::Vec3(item.body.velocity)));
    v.push((format_in!(arena, "{prefix}.creation_tick").leak(), Value::Num(item.creation_tick.u32() as _)));
}





fn parse_item(str: &str) -> Item {
    let (split_pos, _) = str.bytes().enumerate().rev().find(|x| x.1 == b'x').unwrap();
    let (ident, amount) = str.split_at(split_pos);
    let ident = ident.trim();

    let kind = *ItemKind::ALL.iter().find(|f| f.to_string() == ident).unwrap();
    let amount : u32 = amount[1..].parse().unwrap();

    let item = Item { amount, kind };
    item
}


fn parse_dropped_item(hm: &HashMap<&str, Value>, buf: &mut sti::string::String<&Arena>) -> Option<DroppedItem> {
    let buf_len = buf.len();

    buf.push(".item");
    let Some(&value) = hm.get(buf.as_str())
    else { return None };

    let item = parse_item(value.as_str());

    // parse the body
    unsafe { buf.inner_mut().set_len(buf_len); }
    buf.push(".body.position");
    let position = hm[buf.as_str()].as_vec3();

    unsafe { buf.inner_mut().set_len(buf_len); }
    buf.push(".body.velocity");
    let velocity = hm[buf.as_str()].as_vec3();


    unsafe { buf.inner_mut().set_len(buf_len); }
    buf.push(".creation_tick");
    let creation_tick = hm[buf.as_str()].as_u32();

    unsafe { buf.inner_mut().set_len(buf_len); }

    let dropped_item = DroppedItem {
        item,
        body: PhysicsBody { position: position.as_dvec3(), velocity, aabb_dims: Vec3::splat(DROPPED_ITEM_SCALE) },
        creation_tick: Tick(creation_tick),
    };

    Some(dropped_item)
}



