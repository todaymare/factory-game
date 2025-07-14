use glam::IVec3;

use crate::{crafting::{Recipe, FURNACE_RECIPES}, directions::CardinalDirection, items::{Item, ItemKind}, mesh::Mesh};

use super::inventory::{SlotKind, SlotMeta, StructureInventory};

#[derive(Debug)]
pub struct Structure {
    pub position: IVec3,
    pub direction: CardinalDirection,
    pub data: StructureData,

    pub inventory: Option<StructureInventory>,

    pub is_asleep: bool,
}


#[derive(PartialEq, Eq, Hash, Debug)]
pub enum StructureData {
    Quarry {
        current_progress: u32,
    },

    Inserter {
        state: InserterState,
        filter: Option<ItemKind>,
    },

    Chest,
    Silo,
    Belt,

    Splitter {
        priority: [u8; 2],
    },


    Assembler {
        recipe: Option<Recipe>,
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
    Silo,
    Belt,
    Splitter,
    Assembler,
    Furnace,
}


impl StructureData {
    fn from_kind(kind: StructureKind) -> (Self, Option<StructureInventory>) {
        match kind {

            StructureKind::Quarry => {
                const SLOTS : &[SlotMeta] = &[SlotMeta::new(1, SlotKind::Output)];
                (Self::Quarry { current_progress: 0 }, Some(StructureInventory::new(SLOTS)))
            },


            StructureKind::Inserter => {
                (Self::Inserter { state: InserterState::Searching, filter: None }, None)
            },


            StructureKind::Chest => {
                const SLOTS : &[SlotMeta] = &[SlotMeta::new(u32::MAX, SlotKind::Storage); 3*3];
                (Self::Chest, Some(StructureInventory::new(SLOTS)))
            },


            StructureKind::Silo => {
                const SLOTS : &[SlotMeta] = &[SlotMeta::new(u32::MAX, SlotKind::Storage); 6*6];
                (Self::Silo, Some(StructureInventory::new(SLOTS)))
            },


            StructureKind::Belt => {
                const SLOTS : &[SlotMeta] = &[SlotMeta::new(1, SlotKind::Storage); 4];
                (Self::Belt, Some(StructureInventory::new(SLOTS)))
            },
            StructureKind::Splitter => {
                const SLOTS : &[SlotMeta] = &[SlotMeta::new(1, SlotKind::Storage); 8];
                (Self::Splitter { priority: [0; 2] }, Some(StructureInventory::new(SLOTS)))
            },


            StructureKind::Assembler => (Self::Assembler { recipe: None }, None),


            StructureKind::Furnace => (Self::Furnace { input: None, output: None }, None),
        }
    }


    pub fn as_kind(&self) -> StructureKind {
        match self {
            StructureData::Quarry { .. } => StructureKind::Quarry,
            StructureData::Inserter { .. } => StructureKind::Inserter,
            StructureData::Chest { .. } => StructureKind::Chest,
            StructureData::Silo { .. } => StructureKind::Silo ,
            StructureData::Belt { .. } => StructureKind::Belt,
            StructureData::Splitter { .. } => StructureKind::Splitter,
            StructureData::Assembler { .. } => StructureKind::Assembler,
            StructureData::Furnace { .. } => StructureKind::Furnace,
        }
    }
}



impl Structure {
    pub fn from_kind(kind: StructureKind, pos: IVec3, direction: CardinalDirection) -> Structure {
        let (data, inv) = StructureData::from_kind(kind);
        Structure {
            data,
            position: pos,
            direction,
            is_asleep: true,
            inventory: inv,
        }
    }


    pub fn zero_zero(&self) -> IVec3 {
        self.position - self.data.as_kind().origin(self.direction)
    }


    pub fn can_accept(&self, item: Item) -> bool {
        match self.data {
            StructureData::Furnace { input, output } => {
                if let Some(input) = input {
                    input.kind == item.kind && input.amount < 5 && input.amount + item.amount <= input.kind.max_stack_size()
                } else if let Some(output) = output {
                    let curr_recipe = FURNACE_RECIPES.iter().find(|x| x.result.kind == output.kind).unwrap();
                    let input = curr_recipe.requirements[0];
                    input.kind == item.kind && input.amount + item.amount <= input.kind.max_stack_size()
                } else {
                    FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == item.kind).is_some()
                }
            }


            _ => {
                let Some(inventory) = &self.inventory
                else { return false };

                inventory.can_accept(item)

            }
        }
    }


    pub fn can_accept_from_player(&self, item: Item) -> bool {
        match self.data {
            StructureData::Furnace { input, output } => {
                if let Some(input) = input {
                    input.kind == item.kind && input.amount + item.amount <= input.kind.max_stack_size()
                } else if let Some(output) = output {
                    let curr_recipe = FURNACE_RECIPES.iter().find(|x| x.result.kind == output.kind).unwrap();
                    let input = curr_recipe.requirements[0];
                    input.kind == item.kind && input.amount + item.amount <= input.kind.max_stack_size()
                } else {
                    FURNACE_RECIPES.iter().find(|x| x.requirements[0].kind == item.kind).is_some()
                }
            }


            _ => {
                let Some(inventory) = &self.inventory
                else { return false };

                inventory.can_accept(item)

            }
        }
    }

    pub fn give_item(&mut self, item: Item) {
        match &mut self.data {
            StructureData::Furnace { input, .. } => {
                if let Some(input) = input {
                    input.amount += item.amount;
                } else {
                    *input = Some(item);
                }
            },


            _ => {
                let Some(inventory) = &mut self.inventory
                else { panic!("tried to give an item '{item:?}' to a structure with no inventory") };

                inventory.give_item(item);

            }
        }
    }


    pub fn available_items_len(&self) -> usize {
        match self.data {
            StructureData::Furnace { output, .. } => output.is_some() as _,


            _ => {
                let Some(inventory) = &self.inventory
                else { return 0 };

                inventory.outputs_len()

            }
        }
    }


    pub fn available_item(&self, index: usize) -> &Option<Item> {
        match &self.data {
            StructureData::Furnace { output, .. } => &output,

            _ => {
                let Some(inventory) = &self.inventory
                else { panic!("tried to view an item from a structure with no inventory") };

                inventory.output(index).0

            }
        }

    }


    pub fn try_take(&mut self, index: usize, max: u32) -> Option<Item> {
        match &mut self.data {
            StructureData::Furnace { output, .. } => {
                if let Some(output_item) = output {
                    let amount = max.min(output_item.amount);
                    output_item.amount -= amount;
                    let mut item = *output_item;
                    item.amount = amount;

                    if output_item.amount == 0 { *output = None };

                    return Some(item)
                }

                None
            }


            _ => {
                let Some(inventory) = &mut self.inventory
                else { panic!("tried to take an item from a structure with no inventory") };

                inventory.try_take(index, max)

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

            StructureKind::Silo => {
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
            StructureKind::Silo => rotate_block_vector(dir, IVec3::new(2, 0, 1)),
            StructureKind::Belt => rotate_block_vector(dir, IVec3::new(0, 0, 0)),
            StructureKind::Splitter => rotate_block_vector(dir, IVec3::new(0, 0, 0)),
            StructureKind::Assembler => rotate_block_vector(dir, IVec3::new(2, 0, 1)),
            StructureKind::Furnace => rotate_block_vector(dir, IVec3::new(2, 0, 1)),
        }
    }


    pub fn create_mesh(self, device: &wgpu::Device) -> Mesh {
        match self {
            StructureKind::Quarry => Mesh::from_vmf(device, "assets/models/quarry.vmf"),
            StructureKind::Inserter => Mesh::from_vmf(device, "assets/models/inserter.vmf"),
            StructureKind::Chest => Mesh::from_vmf(device, "assets/models/chest.vmf"),
            StructureKind::Silo => Mesh::from_vmf(device, "assets/models/silo.vmf"),
            StructureKind::Belt => Mesh::from_vmf(device, "assets/models/belt.vmf"),
            StructureKind::Splitter => Mesh::from_vmf(device, "assets/models/splitter.vmf"),
            StructureKind::Assembler => Mesh::from_vmf(device, "assets/models/assembler.vmf"),
            StructureKind::Furnace => Mesh::from_vmf(device, "assets/models/furnace.vmf"),
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




