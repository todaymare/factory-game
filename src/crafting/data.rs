//
//
//
// AUTO GENERATED CODE
// CHECK `scripts/generate_recipes.py` for more info
//
//
//
use crate::{items::{Item, ItemKind}, structures::{inventory::{Filter, SlotKind, SlotMeta}, strct::StructureKind}, voxel_world::voxel::Voxel, constants::TICKS_PER_SECOND};use super::Recipe;
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
    Recipe {
        requirements: &[Item::new(ItemKind::IronPlate, 5)],
        result: Item::new(ItemKind::SteelPlate, 1),
        time: TICKS_PER_SECOND * 5,
    },
];
pub const RECIPES : &'static [Recipe] = &[
    Recipe {
        requirements: &[Item::new(ItemKind::Voxel(Voxel::Stone), 2)],
        result: Item::new(ItemKind::Brick, 1),
        time: TICKS_PER_SECOND / 2,
    },
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
        requirements: &[Item::new(ItemKind::ElectronicsKit, 2), Item::new(ItemKind::IronPlate, 4)],
        result: Item::new(ItemKind::CircuitBoard, 1),
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
        requirements: &[Item::new(ItemKind::SteelPlate, 8), Item::new(ItemKind::Brick, 32)],
        result: Item::new(ItemKind::Structure(StructureKind::SteelFurnace), 1),
        time: TICKS_PER_SECOND * 12,
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
    Recipe {
        requirements: &[Item::new(ItemKind::SteelPlate, 30), Item::new(ItemKind::CircuitBoard, 20), Item::new(ItemKind::Brick, 50)],
        result: Item::new(ItemKind::Radar, 1),
        time: TICKS_PER_SECOND / 10,
    },
];
pub fn crafting_recipe_inventory(index: usize) -> &'static [SlotMeta] {
    match index {
        0 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(4, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        1 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(4, SlotKind::Input { filter: Filter::ItemKind(ItemKind::IronPlate) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        2 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(2, SlotKind::Input { filter: Filter::ItemKind(ItemKind::IronPlate) }),
                SlotMeta::new(4, SlotKind::Output),
            ];
            SLOTS
        },
        3 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(2, SlotKind::Input { filter: Filter::ItemKind(ItemKind::CopperPlate) }),
                SlotMeta::new(6, SlotKind::Output),
            ];
            SLOTS
        },
        4 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(4, SlotKind::Input { filter: Filter::ItemKind(ItemKind::IronRod) }),
                SlotMeta::new(2, SlotKind::Input { filter: Filter::ItemKind(ItemKind::IronGearWheel) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        5 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(6, SlotKind::Input { filter: Filter::ItemKind(ItemKind::CopperWire) }),
                SlotMeta::new(2, SlotKind::Input { filter: Filter::ItemKind(ItemKind::CopperPlate) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        6 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(4, SlotKind::Input { filter: Filter::ItemKind(ItemKind::ElectronicsKit) }),
                SlotMeta::new(8, SlotKind::Input { filter: Filter::ItemKind(ItemKind::IronPlate) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        7 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(2, SlotKind::Input { filter: Filter::ItemKind(ItemKind::IronGearWheel) }),
                SlotMeta::new(8, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(6, SlotKind::Output),
            ];
            SLOTS
        },
        8 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(8, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Structure(StructureKind::Belt)) }),
                SlotMeta::new(2, SlotKind::Input { filter: Filter::ItemKind(ItemKind::ElectronicsKit) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        9 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(4, SlotKind::Input { filter: Filter::ItemKind(ItemKind::IronGearWheel) }),
                SlotMeta::new(32, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        10 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(8, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Structure(StructureKind::Chest)) }),
                SlotMeta::new(128, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        11 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(2, SlotKind::Input { filter: Filter::ItemKind(ItemKind::MechanicalComponent) }),
                SlotMeta::new(2, SlotKind::Input { filter: Filter::ItemKind(ItemKind::ElectronicsKit) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        12 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(32, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(8, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Coal) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        13 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(16, SlotKind::Input { filter: Filter::ItemKind(ItemKind::SteelPlate) }),
                SlotMeta::new(64, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Brick) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        14 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(8, SlotKind::Input { filter: Filter::ItemKind(ItemKind::MechanicalComponent) }),
                SlotMeta::new(24, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Voxel(Voxel::Stone)) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        15 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(6, SlotKind::Input { filter: Filter::ItemKind(ItemKind::MechanicalComponent) }),
                SlotMeta::new(4, SlotKind::Input { filter: Filter::ItemKind(ItemKind::ElectronicsKit) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        16 => {
            const SLOTS : &[SlotMeta] = &[
                SlotMeta::new(60, SlotKind::Input { filter: Filter::ItemKind(ItemKind::SteelPlate) }),
                SlotMeta::new(40, SlotKind::Input { filter: Filter::ItemKind(ItemKind::CircuitBoard) }),
                SlotMeta::new(100, SlotKind::Input { filter: Filter::ItemKind(ItemKind::Brick) }),
                SlotMeta::new(2, SlotKind::Output),
            ];
            SLOTS
        },
        _ => unreachable!(),
    }
}