use glam::IVec3;

use crate::{directions::CardinalDirection, items::{Item, ItemKind}, mesh::Mesh, Tick};

use super::Slot;

pub struct Structure {
    pub position: IVec3,
    pub direction: CardinalDirection,
    pub data: StructureData,

    pub input : Option<Vec<Slot>>,
    pub output: Option<Slot>,

    pub is_asleep: bool,
}


#[derive(PartialEq, Eq, Hash, Debug)]
pub enum StructureData {
    Quarry {
        current_progress: usize,
    },
    Inserter,
}


#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum StructureKind {
    Quarry,
    Inserter,
}


impl StructureData {
    pub fn from_kind(kind: StructureKind) -> Self {
        match kind {
            StructureKind::Quarry => Self::Quarry { current_progress: 0 },
            StructureKind::Inserter => Self::Inserter,
        }
    }


    pub fn as_kind(&self) -> StructureKind {
        match self {
            StructureData::Quarry { .. } => StructureKind::Quarry,
            StructureData::Inserter => StructureKind::Inserter,
        }
    }
}



impl Structure {
    pub fn from_kind(kind: StructureKind, pos: IVec3, direction: CardinalDirection) -> Structure {
        match kind {
            StructureKind::Quarry => {
                Structure {
                    data: StructureData::from_kind(StructureKind::Quarry),
                    input: None,
                    output: Some(Slot { item: None, expected: None, max: 1 }),
                    position: pos,
                    direction,
                    is_asleep: true,
                }
            },


            StructureKind::Inserter => {
                Structure {
                    position: pos,
                    direction,
                    data: StructureData::Inserter,
                    input: None,
                    output: None,
                    is_asleep: true,
                }
            },
        }
    }


    pub fn zero_zero(&self) -> IVec3 {
        self.position - self.data.as_kind().origin(self.direction)
    }


    pub fn try_take(&mut self) -> Option<Item> {
        let slot = self.output.as_mut()?;
        let item = slot.item.as_mut()?;

        if item.amount == 1 {
            let item = *item;
            slot.item = None;
            return Some(item);
        } else {
            item.amount -= 1;

            let mut item = *item;
            item.amount = 1;

            return Some(item)
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
        }
    }


    pub fn origin(self, dir: CardinalDirection) -> IVec3 {
        match self {
            StructureKind::Quarry => rotate_block_vector(dir, IVec3::new(4, 0, 2)),
            StructureKind::Inserter => rotate_block_vector(dir, IVec3::new(2, 0, 0)),
        }
    }


    pub fn mesh(self) -> Mesh {
        match self {
            StructureKind::Quarry => Mesh::from_obj("quarry.obj"),
            StructureKind::Inserter => Mesh::from_obj("inserter.obj"),
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




