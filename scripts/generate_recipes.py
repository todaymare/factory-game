furnace_recipes = {
    "IronPlate": {
        "requirements": [("IronOre", 1)],
        "amount": 1,
        "time": 1,
    },
    "CopperPlate": {
        "requirements": [("CopperOre", 1)],
        "amount": 1,
        "time": 1,
    },
    "SteelPlate": {
        "requirements": [("IronPlate", 5)],
        "amount": 1,
        "time": 5,
    },
}

recipes = {
    "Brick": {
        "requirements": [("Voxel(Voxel::Stone)", 2)],
        "amount": 1,
        "time": 0.5,
    },
    "IronGearWheel": {
        "requirements": [("IronPlate", 2)],
        "amount": 1,
        "time": 0.5,
    },
    "IronRod": {
        "requirements": [("IronPlate", 1)],
        "amount": 2,
        "time": 0.5,
    },
    "CopperWire": {
        "requirements": [("CopperPlate", 1)],
        "amount": 3,
        "time": 0.5,
    },
    "MechanicalComponent": {
        "requirements": [
            ("IronRod", 2),
            ("IronGearWheel", 1),
        ],
        "amount": 1,
        "time": 1,
    },
    "ElectronicsKit": {
        "requirements": [
            ("CopperWire", 3),
            ("CopperPlate", 1),
        ],
        "amount": 1,
        "time": 1,
    },
    "CircuitBoard": {
        "requirements": [
            ("ElectronicsKit", 2),
            ("IronPlate", 4),
        ],
        "amount": 1,
        "time": 1,
    },
    "Structure(StructureKind::Belt)": {
        "requirements": [
            ("IronGearWheel", 1),
            ("Voxel(Voxel::Stone)", 4),
        ],
        "amount": 3,
        "time": 2,
    },
    "Structure(StructureKind::Splitter)": {
        "requirements": [
            ("Structure(StructureKind::Belt)", 4),
            ("ElectronicsKit", 1),
        ],
        "amount": 1,
        "time": 2,
    },
    "Structure(StructureKind::Chest)": {
        "requirements": [
            ("IronGearWheel", 2),
            ("Voxel(Voxel::Stone)", 16),
        ],
        "amount": 1,
        "time": 2,
    },
    "Structure(StructureKind::Silo)": {
        "requirements": [
            ("Structure(StructureKind::Chest)", 4),
            ("Voxel(Voxel::Stone)", 64),
        ],
        "amount": 1,
        "time": 2,
    },
    "Structure(StructureKind::Inserter)": {
        "requirements": [
            ("MechanicalComponent", 1),
            ("ElectronicsKit", 1),
        ],
        "amount": 1,
        "time": 2,
    },
    "Structure(StructureKind::Furnace)": {
        "requirements": [
            ("Voxel(Voxel::Stone)", 16),
            ("Coal", 4),
        ],
        "amount": 1,
        "time": 2,
    },
    "Structure(StructureKind::SteelFurnace)": {
        "requirements": [
            ("SteelPlate", 8),
            ("Brick", 32),
        ],
        "amount": 1,
        "time": 12,
    },
    "Structure(StructureKind::Quarry)": {
        "requirements": [
            ("MechanicalComponent", 4),
            ("Voxel(Voxel::Stone)", 12),
        ],
        "amount": 1,
        "time": 2,
    },
    "Structure(StructureKind::Assembler)": {
        "requirements": [
            ("MechanicalComponent", 3),
            ("ElectronicsKit", 2),
        ],
        "amount": 1,
        "time": 2,
    },

    "Radar": {
        "requirements": [
            ("SteelPlate", 30),
            ("CircuitBoard", 20),
            ("Brick", 50),
        ],
        "amount": 1,
        "time": 0.1,
    },
}










# hiding my chatgpt shame

def rust_item(kind, amount):
    return f"Item::new(ItemKind::{kind}, {amount})"

def rust_time_expr(time):
    if time == 1:
        return "TICKS_PER_SECOND"
    elif time < 1:
        denominator = round(1 / time)
        return f"TICKS_PER_SECOND / {denominator}"
    else:
        multiplier = round(time)
        return f"TICKS_PER_SECOND * {multiplier}"

def rust_recipe(name, data):
    requirements = ", ".join([rust_item(r[0], r[1]) for r in data["requirements"]])
    result = rust_item(name, data["amount"])
    time = rust_time_expr(data["time"])
    return f"""    Recipe {{
        requirements: &[{requirements}],
        result: {result},
        time: {time},
    }}"""

def generate_recipes(recipes):
    lines = ["pub const RECIPES : &'static [Recipe] = &["]
    for name, data in recipes.items():
        lines.append(rust_recipe(name, data) + ",")
    lines.append("];")
    return "\n".join(lines)


def generate_furnace_recipes(recipes):
    lines = ["pub const FURNACE_RECIPES : &'static [Recipe] = &["]
    for name, data in recipes.items():
        lines.append(rust_recipe(name, data) + ",")
    lines.append("];")
    return "\n".join(lines)

def generate_slot_meta(index, data):
    lines = [f"        {index} => {{",
             "            const SLOTS : &[SlotMeta] = &["]
    for req in data["requirements"]:
        kind = req[0]
        amount = req[1]
        slots = 2 * amount
        lines.append(f"                SlotMeta::new({slots}, SlotKind::Input {{ filter: Filter::ItemKind(ItemKind::{kind}) }}),")
    amount = data["amount"] * 2
    lines.append(f"                SlotMeta::new({amount}, SlotKind::Output),")
    lines.append("            ];")
    lines.append("            SLOTS")
    lines.append("        },")
    return "\n".join(lines)

def generate_slot_match(recipes):
    lines = ["pub fn crafting_recipe_inventory(index: usize) -> &'static [SlotMeta] {",
             "    match index {"]
    for i, (_, data) in enumerate(recipes.items()):
        lines.append(generate_slot_meta(i, data))
    lines.append("        _ => unreachable!(),")
    lines.append("    }")
    lines.append("}")
    return "\n".join(lines)

# Combine everything
string = ""
string += "//\n"
string += "//\n"
string += "//\n"
string += "// AUTO GENERATED CODE\n"
string += "// CHECK `scripts/generate_recipes.py` for more info\n"
string += "//\n"
string += "//\n"
string += "//\n"
string += "use crate::{items::{Item, ItemKind}, structures::{inventory::{Filter, SlotKind, SlotMeta}, strct::StructureKind}, voxel_world::voxel::Voxel, constants::TICKS_PER_SECOND};"
string += "use super::Recipe;"
string += "\n"
string += generate_furnace_recipes(furnace_recipes)
string += "\n"
string += generate_recipes(recipes)
string += "\n"
string += generate_slot_match(recipes)

open("src/crafting/data.rs", "w").write(string)
