use crate::{items::{Item, ItemKind}, structures::strct::StructureKind, voxel_world::voxel::Voxel, TICKS_PER_SECOND};


pub const FURNACE_RECIPES : &'static [Recipe] = &[
    Recipe {
        requirements: &[Item::new(ItemKind::IronOre, 1)],
        result: Item::new(ItemKind::IronPlate, 1),
        time: TICKS_PER_SECOND,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::CopperOre, 1)],
        result: Item::new(ItemKind::CopperPlate, 1),
        time: TICKS_PER_SECOND,
    },
];


pub const RECIPES : &'static [Recipe] = &[

    Recipe {
        requirements: &[Item::new(ItemKind::IronPlate, 2)],
        result: Item::new(ItemKind::IronGearWheel, 1),
        time: TICKS_PER_SECOND/2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::IronPlate, 1)],
        result: Item::new(ItemKind::IronRod, 2),
        time: TICKS_PER_SECOND/2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::CopperPlate, 1)],
        result: Item::new(ItemKind::CopperWire, 3),
        time: TICKS_PER_SECOND/2,
    },


    Recipe {
        requirements: &[Item::new(ItemKind::IronRod, 2), Item::new(ItemKind::IronGearWheel, 1)],
        result: Item::new(ItemKind::MechanicalComponent, 1),
        time: TICKS_PER_SECOND,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::CopperWire, 3), Item::new(ItemKind::CopperPlate, 1)],
        result: Item::new(ItemKind::ElectronicsKit, 1),
        time: TICKS_PER_SECOND,
    },

    Recipe {
        requirements: &[Item::new(ItemKind::IronGearWheel, 1), Item::new(ItemKind::Voxel(Voxel::Stone), 4)],
        result: Item::new(ItemKind::Structure(StructureKind::Belt), 3),
        time: TICKS_PER_SECOND*2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::Structure(StructureKind::Belt), 4), Item::new(ItemKind::ElectronicsKit, 1)],
        result: Item::new(ItemKind::Structure(StructureKind::Splitter), 3),
        time: TICKS_PER_SECOND*2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::IronGearWheel, 2), Item::new(ItemKind::Voxel(Voxel::Stone), 16)],
        result: Item::new(ItemKind::Structure(StructureKind::Chest), 1),
        time: TICKS_PER_SECOND*2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::Structure(StructureKind::Chest), 4), Item::new(ItemKind::Voxel(Voxel::Stone), 64)],
        result: Item::new(ItemKind::Structure(StructureKind::Silo), 1),
        time: TICKS_PER_SECOND*2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::MechanicalComponent, 1), Item::new(ItemKind::ElectronicsKit, 1)],
        result: Item::new(ItemKind::Structure(StructureKind::Inserter), 1),
        time: TICKS_PER_SECOND*2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::Voxel(Voxel::Stone), 16), Item::new(ItemKind::Coal, 4)],
        result: Item::new(ItemKind::Structure(StructureKind::Furnace), 1),
        time: TICKS_PER_SECOND*2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::MechanicalComponent, 4), Item::new(ItemKind::Voxel(Voxel::Stone), 12)],
        result: Item::new(ItemKind::Structure(StructureKind::Quarry), 1),
        time: TICKS_PER_SECOND*2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::MechanicalComponent, 3), Item::new(ItemKind::ElectronicsKit, 2)],
        result: Item::new(ItemKind::Structure(StructureKind::Assembler), 1),
        time: TICKS_PER_SECOND*2,
    },
];


#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct Recipe {
    pub requirements: &'static [Item],
    pub result: Item,
    pub time: u32,
}


