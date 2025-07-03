//
//
//
// AUTO GENERATED CODE
// CHECK `scripts/generate_recipes.py` for more info
//
//
//
use crate::{constants::TICKS_PER_SECOND, items::{Item, ItemKind}, structures::{inventory::{SlotKind, SlotMeta}, strct::StructureKind}, voxel_world::voxel::Voxel};
use super::Recipe;

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
        time: TICKS_PER_SECOND / 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::IronPlate, 1)],
        result: Item::new(ItemKind::IronRod, 2),
        time: TICKS_PER_SECOND / 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::CopperPlate, 1)],
        result: Item::new(ItemKind::CopperWire, 3),
        time: TICKS_PER_SECOND / 2,
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
        time: TICKS_PER_SECOND * 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::Structure(StructureKind::Belt), 4), Item::new(ItemKind::ElectronicsKit, 1)],
        result: Item::new(ItemKind::Structure(StructureKind::Splitter), 1),
        time: TICKS_PER_SECOND * 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::IronGearWheel, 2), Item::new(ItemKind::Voxel(Voxel::Stone), 16)],
        result: Item::new(ItemKind::Structure(StructureKind::Chest), 1),
        time: TICKS_PER_SECOND * 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::Structure(StructureKind::Chest), 4), Item::new(ItemKind::Voxel(Voxel::Stone), 64)],
        result: Item::new(ItemKind::Structure(StructureKind::Silo), 1),
        time: TICKS_PER_SECOND * 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::MechanicalComponent, 1), Item::new(ItemKind::ElectronicsKit, 1)],
        result: Item::new(ItemKind::Structure(StructureKind::Inserter), 1),
        time: TICKS_PER_SECOND * 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::Voxel(Voxel::Stone), 16), Item::new(ItemKind::Coal, 4)],
        result: Item::new(ItemKind::Structure(StructureKind::Furnace), 1),
        time: TICKS_PER_SECOND * 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::MechanicalComponent, 4), Item::new(ItemKind::Voxel(Voxel::Stone), 12)],
        result: Item::new(ItemKind::Structure(StructureKind::Quarry), 1),
        time: TICKS_PER_SECOND * 2,
    },
    Recipe {
        requirements: &[Item::new(ItemKind::MechanicalComponent, 3), Item::new(ItemKind::ElectronicsKit, 2)],
        result: Item::new(ItemKind::Structure(StructureKind::Assembler), 1),
        time: TICKS_PER_SECOND * 2,
    },
];
pub fn crafting_recipe_inventory(index: usize) -> &'static [SlotMeta] {
    match index {
        0 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(4, SlotKind::Input { filter: Some(ItemKind::IronPlate) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        1 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(2, SlotKind::Input { filter: Some(ItemKind::IronPlate) }),
                SlotMeta::new(4, SlotKind::Output),
            ];
            SLOTS
        },
        2 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(2, SlotKind::Input { filter: Some(ItemKind::CopperPlate) }),
                SlotMeta::new(6, SlotKind::Output),
            ];
            SLOTS
        },
        3 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(4, SlotKind::Input { filter: Some(ItemKind::IronRod) }),
                SlotMeta::new(2, SlotKind::Input { filter: Some(ItemKind::IronGearWheel) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        4 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(6, SlotKind::Input { filter: Some(ItemKind::CopperWire) }),
                SlotMeta::new(2, SlotKind::Input { filter: Some(ItemKind::CopperPlate) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        5 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(2, SlotKind::Input { filter: Some(ItemKind::IronGearWheel) }),
                SlotMeta::new(8, SlotKind::Input { filter: Some(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(6, SlotKind::Output),
            ];
            SLOTS
        },
        6 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(8, SlotKind::Input { filter: Some(ItemKind::Structure(StructureKind::Belt)) }),
                SlotMeta::new(2, SlotKind::Input { filter: Some(ItemKind::ElectronicsKit) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        7 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(4, SlotKind::Input { filter: Some(ItemKind::IronGearWheel) }),
                SlotMeta::new(32, SlotKind::Input { filter: Some(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        8 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(8, SlotKind::Input { filter: Some(ItemKind::Structure(StructureKind::Chest)) }),
                SlotMeta::new(128, SlotKind::Input { filter: Some(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        9 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(2, SlotKind::Input { filter: Some(ItemKind::MechanicalComponent) }),
                SlotMeta::new(2, SlotKind::Input { filter: Some(ItemKind::ElectronicsKit) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        10 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(32, SlotKind::Input { filter: Some(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(8, SlotKind::Input { filter: Some(ItemKind::Coal) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        11 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(8, SlotKind::Input { filter: Some(ItemKind::MechanicalComponent) }),
                SlotMeta::new(24, SlotKind::Input { filter: Some(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        12 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(6, SlotKind::Input { filter: Some(ItemKind::MechanicalComponent) }),
                SlotMeta::new(4, SlotKind::Input { filter: Some(ItemKind::ElectronicsKit) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        _ => unreachable!(),
    }
}
