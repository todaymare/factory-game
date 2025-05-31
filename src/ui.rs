use std::{fmt::Write, ops::Bound};

use glam::{Vec2, Vec3};
use glfw::CursorMode;

use crate::{renderer::Renderer, structures::strct::{InserterState, StructureData}, voxel_world::split_world_pos, Game, PLAYER_REACH};

pub enum UILayer {
    Gameplay,
}


impl UILayer {
    pub fn capture_mode(&self) -> CursorMode {
        match self {
            UILayer::Gameplay => CursorMode::Disabled,
        }
    }


    pub fn render(&mut self, game: &mut Game, renderer: &Renderer, dt: f32) {
        match self {
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

                    renderer.draw_text(&text, Vec2::ZERO, 0.4, Vec3::ONE);
                }
            },
        }
    }
}
