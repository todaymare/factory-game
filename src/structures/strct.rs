use glam::IVec3;

use crate::{crafting::{FURNACE_RECIPES, RECIPES}, directions::CardinalDirection, items::{Item, ItemKind}, mesh::Mesh};

use super::Crafter;

#[derive(Debug)]
pub struct Structure {
    pub position: IVec3,
    pub direction: CardinalDirection,
    pub data: StructureData,

    pub is_asleep: bool,
}


#[derive(PartialEq, Eq, Hash, Debug)]
pub enum StructureData {
    Quarry {
        current_progress: u32,
        output: Option<Item>,
    },

    Inserter {
        state: InserterState,
        filter: Option<ItemKind>,
    },

    Chest {
        inventory: Vec<Option<Item>>,
    },


    Belt {
        inventory: [[Option<Item>; 2]; 2],
    },


    Splitter {
        inventory: [[[Option<Item>; 2]; 2]; 2],
        priority: [u8; 2],
    },


    Assembler {
        crafter: Crafter,
    },

    Furnace {
        input: Option<Item>,
        output: Option<Item>,
    }

}


#[derive(PartialEq, Eq, Hash, Debug)]
pub enum InserterState {
    Searching,
    Placing(Item),
}


#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum StructureKind {
    Quarry,
    Inserter,
    Chest,
    Belt,
    Splitter,
    Assembler,
    Furnace,
}


impl StructureData {
    pub fn from_kind(kind: StructureKind) -> Self {
        match kind {
            StructureKind::Quarry => Self::Quarry { current_progress: 0, output: None },
            StructureKind::Inserter => Self::Inserter { state: InserterState::Searching, filter: None },
            StructureKind::Chest => Self::Chest { inventory: vec![None; 6*6] },
            StructureKind::Belt => Self::Belt { inventory: [[None; 2]; 2] },
            StructureKind::Splitter => Self::Splitter { inventory: [[[None; 2]; 2]; 2], priority: [0, 0] },
            StructureKind::Assembler => Self::Assembler { crafter: Crafter::from_recipe(RECIPES[0]) },
            StructureKind::Furnace => Self::Furnace { input: None, output: None },
        }
    }


    pub fn as_kind(&self) -> StructureKind {
        match self {
            StructureData::Quarry { .. } => StructureKind::Quarry,
            StructureData::Inserter { .. } => StructureKind::Inserter,
            StructureData::Chest { .. } => StructureKind::Chest,
            StructureData::Belt { .. } => StructureKind::Belt,
            StructureData::Splitter { .. } => StructureKind::Splitter,
            StructureData::Assembler { .. } => StructureKind::Assembler,
            StructureData::Furnace { .. } => StructureKind::Furnace,
        }
    }
}



impl Structure {
    pub fn from_kind(kind: StructureKind, pos: IVec3, direction: CardinalDirection) -> Structure {
        Structure {
            data: StructureData::from_kind(kind),
            position: pos,
            direction,
            is_asleep: true,
        }
    }


    pub fn zero_zero(&self) -> IVec3 {
        self.position - self.data.as_kind().origin(self.direction)
    }


    pub fn can_accept(&self, item: Item) -> bool {
        match &self.data {
            StructureData::Quarry { .. } => false,
            StructureData::Inserter { .. } => false,


            StructureData::Chest { inventory } => {
                for slot in inventory {
                    let Some(slot) = slot
                    else { return true; };

                    if slot.kind != item.kind { continue }

                    if slot.amount + item.amount <= slot.kind.max_stack_size() {
                        return true;
                    }
                }

                false
            },


            StructureData::Belt { inventory } => {
                for arr in 0..inventory.len() {
                    let arr = &inventory[arr];
                    for i in 0..arr.len() {
                        if arr[i].is_none() {
                            return true;
                        }
                    }
                }
                false
            },


            StructureData::Splitter { inventory, priority } => {
                for inventory in inventory {
                    for arr in 0..inventory.len() {
                        let arr = &inventory[arr];
                        for i in 0..arr.len() {
                            if arr[i].is_none() {
                                return true;
                            }
                        }
                    }
                }
                false
            },


            StructureData::Assembler { crafter } => {
                for i in 0..crafter.inventory.len() {
                    let inventory_item = crafter.inventory[i];
                    if inventory_item.kind != item.kind { continue }

                    let recipe_amount = crafter.recipe.requirements[i].amount;
                    if inventory_item.amount < recipe_amount * 2 {
                        return true
                    }
                    return false
                }

                false
            }


            StructureData::Furnace { .. } => {
                FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == item.kind).is_some()
            }
        }
    }


    pub fn give_item(&mut self, item: Item) {
        assert!(self.can_accept(item));
        match &mut self.data {
            StructureData::Chest { inventory } => {
                for slot in inventory {
                    let Some(slot) = slot
                    else { *slot = Some(item); return; };

                    if slot.kind != item.kind { continue }

                    if slot.amount + item.amount <= slot.kind.max_stack_size() {
                        slot.amount += item.amount;
                        return;
                    }
                }
            },



            StructureData::Belt { inventory } => {
                for inv in inventory {
                    for slot in inv {
                        if slot.is_none() {
                            *slot = Some(item);
                            return;
                        }
                    }
                }
            },


            StructureData::Splitter { inventory, .. } => {
                for inv in inventory {
                    for inv in inv {
                        for slot in inv {
                            if slot.is_none() {
                                *slot = Some(item);
                                return;
                            }
                        }
                    }
                }
            },


            StructureData::Assembler { crafter } => {
                for slot in &mut crafter.inventory {
                    if slot.kind == item.kind {
                        slot.amount += item.amount;
                    }
                }
            },

            StructureData::Furnace { input, .. } => {
                if let Some(input) = input {
                    input.amount += item.amount;
                } else {
                    *input = Some(item);
                }
            },



            _ => unreachable!(),
        }
    }


    pub fn available_items_len(&self) -> usize {
        match &self.data {
            StructureData::Quarry { output, .. } => output.is_some() as usize,
            StructureData::Inserter { .. } => 0,
            StructureData::Chest { inventory } => inventory.len(),
            StructureData::Belt { .. } => 4,
            StructureData::Splitter { .. } => 4,
            StructureData::Assembler { crafter } => (crafter.output.amount != 0) as usize,
            StructureData::Furnace { output, .. } => output.is_some() as usize,
        }
    }


    pub fn available_item(&self, index: usize) -> Option<Item> {
        assert!(index < self.available_items_len());
        match &self.data {
            StructureData::Quarry { output, .. } => *output,
            StructureData::Inserter { .. } => None,
            StructureData::Chest { inventory } => inventory[index],
            StructureData::Belt { inventory } => inventory[index/2][index%2],
            StructureData::Splitter { inventory, priority } => inventory[index/4][(index%4)/2][index%2],
            StructureData::Assembler { crafter } => {
                let output = crafter.output;
                if output.amount == 0 { return None };
                Some(output)
            }
            StructureData::Furnace { output, .. } => *output,
        }
    }


    pub fn try_take(&mut self, index: usize) -> Option<Item> {
        match &mut self.data {
            StructureData::Quarry { output, .. } => {
                output.take()
            },

            StructureData::Inserter { .. } => None,
            StructureData::Chest { inventory } => {
                let item = &mut inventory[index];
                let item = item;

                if let Some(slot) = item {
                    slot.amount -= 1;
                    let mut result = *slot;
                    result.amount = 1;

                    if slot.amount == 0 {
                        *item = None;
                    }

                    return Some(result);

                }

                None
            },


            StructureData::Assembler { crafter } => {
                let mut output = crafter.output;
                if output.amount == 0 { return None };
                crafter.output.amount -= 1;
                output.amount = 1;
                Some(output)
            }

            StructureData::Belt { inventory } => {
                let slot = &mut inventory[index / 2][index % 2];
                slot.take()
            },

            StructureData::Splitter { inventory, .. } => {
                let slot = &mut inventory[index/4][(index%4)/ 2][index % 2];
                slot.take()
            },


            StructureData::Furnace { input, output } => {
                if let Some(output_item) = output {
                    output_item.amount -= 1;
                    let mut item = *output_item;
                    item.amount = 1;

                    if output_item.amount == 0 { *output = None };

                    return Some(item)
                }

                None
            }
        }
    }
}


impl StructureKind {
    pub fn item_kind(self) -> ItemKind {
        ItemKind::Structure(self)
    }


    pub fn blocks(self, dir: CardinalDirection) -> &'static [IVec3] {
        macro_rules! blocks_arr {
            ($dir: expr, $($elem: expr),*) => {
                {
                const NORTH : &[IVec3] = &[$($elem),*];
                const SOUTH : &[IVec3] = &[$(rotate_block_vector(CardinalDirection::South, $elem)),*];
                const EAST : &[IVec3] = &[$(rotate_block_vector(CardinalDirection::East, $elem)),*];
                const WEST : &[IVec3] = &[$(rotate_block_vector(CardinalDirection::West, $elem)),*];

                match $dir {
                    CardinalDirection::North => NORTH,
                    CardinalDirection::South => SOUTH,
                    CardinalDirection::East => EAST,
                    CardinalDirection::West => WEST,
                }
                }
                
            };
        }



        match self {
            StructureKind::Quarry => {
                blocks_arr!(dir,
                    IVec3::new(0, 0, 0), IVec3::new(1, 0, 0),
                    IVec3::new(2, 0, 0), IVec3::new(3, 0, 0),
                    IVec3::new(4, 0, 0),

                    IVec3::new(0, 0, 1), IVec3::new(4, 0, 1),
                    IVec3::new(0, 0, 2), IVec3::new(4, 0, 2),
                    IVec3::new(0, 0, 3), IVec3::new(4, 0, 3),

                    IVec3::new(0, 0, 4), IVec3::new(1, 0, 4),
                    IVec3::new(2, 0, 4), IVec3::new(3, 0, 4),
                    IVec3::new(4, 0, 4)
                )
            },

            StructureKind::Inserter => {
                blocks_arr!(dir,
                    IVec3::new(0, 0, 0),
                    IVec3::new(1, 0, 0), 
                    IVec3::new(2, 0, 0)
                )
            }


            StructureKind::Chest => {
                blocks_arr!(dir,
                    IVec3::ZERO)
            }


            StructureKind::Belt => {
                blocks_arr!(dir,
                    IVec3::ZERO)
            }

            StructureKind::Splitter => {
                blocks_arr!(dir,
                    IVec3::new(0, 0, 0),
                    IVec3::new(0, 0, 1)
                )
            }

            StructureKind::Assembler => {
                blocks_arr!(dir,
                    IVec3::new(0, 0, 0), IVec3::new(1, 0, 0), IVec3::new(2, 0, 0),
                    IVec3::new(0, 0, 1), IVec3::new(1, 0, 1), IVec3::new(2, 0, 1),
                    IVec3::new(0, 0, 2), IVec3::new(1, 0, 2), IVec3::new(2, 0, 2),

                    IVec3::new(0, 1, 0), IVec3::new(1, 1, 0), IVec3::new(2, 1, 0),
                    IVec3::new(0, 1, 1), IVec3::new(1, 1, 1), IVec3::new(2, 1, 1),
                    IVec3::new(0, 1, 2), IVec3::new(1, 1, 2), IVec3::new(2, 1, 2),

                    IVec3::new(0, 2, 0), IVec3::new(1, 2, 0), IVec3::new(2, 2, 0),
                    IVec3::new(0, 2, 1), IVec3::new(1, 2, 1), IVec3::new(2, 2, 1),
                    IVec3::new(0, 2, 2), IVec3::new(1, 2, 2), IVec3::new(2, 2, 2)
                )
            }


            StructureKind::Furnace => {
                blocks_arr!(dir,
                    IVec3::new(0, 0, 0), IVec3::new(1, 0, 0), IVec3::new(2, 0, 0),
                    IVec3::new(0, 0, 1), IVec3::new(1, 0, 1), IVec3::new(2, 0, 1),
                    IVec3::new(0, 0, 2), IVec3::new(1, 0, 2), IVec3::new(2, 0, 2),

                    IVec3::new(0, 1, 0), IVec3::new(1, 1, 0), IVec3::new(2, 1, 0),
                    IVec3::new(0, 1, 1), IVec3::new(1, 1, 1), IVec3::new(2, 1, 1),
                    IVec3::new(0, 1, 2), IVec3::new(1, 1, 2), IVec3::new(2, 1, 2),

                    IVec3::new(0, 2, 0), IVec3::new(1, 2, 0), IVec3::new(2, 2, 0),
                    IVec3::new(0, 2, 1), IVec3::new(1, 2, 1), IVec3::new(2, 2, 1),
                    IVec3::new(0, 2, 2), IVec3::new(1, 2, 2), IVec3::new(2, 2, 2)
                )
            }
        }
    }


    pub fn origin(self, dir: CardinalDirection) -> IVec3 {
        match self {
            StructureKind::Quarry => rotate_block_vector(dir, IVec3::new(4, 0, 2)),
            StructureKind::Inserter => rotate_block_vector(dir, IVec3::new(2, 0, 0)),
            StructureKind::Chest => rotate_block_vector(dir, IVec3::new(0, 0, 0)),
            StructureKind::Belt => rotate_block_vector(dir, IVec3::new(0, 0, 0)),
            StructureKind::Splitter => rotate_block_vector(dir, IVec3::new(0, 0, 0)),
            StructureKind::Assembler => rotate_block_vector(dir, IVec3::new(2, 0, 1)),
            StructureKind::Furnace => rotate_block_vector(dir, IVec3::new(2, 0, 1)),
        }
    }


    pub fn mesh(self) -> Mesh {
        match self {
            StructureKind::Quarry => Mesh::from_obj("assets/models/quarry.obj"),
            StructureKind::Inserter => Mesh::from_obj("assets/models/inserter.obj"),
            StructureKind::Chest => Mesh::from_obj("assets/models/block_outline.obj"),
            StructureKind::Belt => Mesh::from_obj("assets/models/belt.obj"),
            StructureKind::Splitter => Mesh::from_obj("assets/models/belt.obj"),
            StructureKind::Assembler => Mesh::from_obj("assets/models/assembler.obj"),
            StructureKind::Furnace => Mesh::from_obj("assets/models/assembler.obj"),
        }
    }
}


pub const fn rotate_block_vector(dir: CardinalDirection, v: IVec3) -> IVec3 {
    match dir {
        CardinalDirection::North => IVec3::new(v.x, v.y, v.z),
        CardinalDirection::East  => IVec3::new(-v.z, v.y, v.x),
        CardinalDirection::South => IVec3::new(-v.x, v.y, -v.z),
        CardinalDirection::West  => IVec3::new(v.z, v.y, -v.x),
    }
}




