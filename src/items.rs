use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use glam::{IVec2, Vec3, Vec4};
use image::{codecs::png::PngDecoder, ImageDecoder};
use rand::random;

use crate::{directions::Direction, mesh::{draw_quad, Mesh}, quad::Quad, renderer::textures::{TextureAtlasBuilder, TextureId}, structures::strct::StructureKind, voxel_world::voxel::VoxelKind, PhysicsBody, Tick, DROPPED_ITEM_SCALE};


#[derive(Clone)]
pub struct DroppedItem {
    pub item: Item,
    pub body: PhysicsBody,
    pub creation_tick: Tick,
}


#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Item {
    pub amount: u32,
    pub kind  : ItemKind,
}


#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum ItemKind {
    Coal,
    CopperOre,
    IronOre,

    Structure(StructureKind),
    Voxel(VoxelKind),

    IronPlate,
    CopperPlate,

    IronGearWheel,
    IronRod,
    CopperWire,
    MechanicalComponent,
    ElectronicsKit,
}


pub struct Assets {
    textures: HashMap<ItemKind, TextureId>,
    models: HashMap<ItemKind, Mesh>,
}


impl Item {
    pub const fn new(kind: ItemKind, amount: u32) -> Self {
        Self {
            amount,
            kind,
        }
    }
}


impl ItemKind {
    pub const ALL : &[ItemKind] = &[
        ItemKind::Voxel(VoxelKind::Dirt),
        ItemKind::Voxel(VoxelKind::Stone),
        ItemKind::CopperOre,
        ItemKind::IronOre,
        ItemKind::Coal,

        ItemKind::IronPlate,
        ItemKind::CopperPlate,

        ItemKind::IronGearWheel,
        ItemKind::IronRod,
        ItemKind::CopperWire,
        ItemKind::MechanicalComponent,
        ItemKind::ElectronicsKit,

        ItemKind::Structure(StructureKind::Quarry),
        ItemKind::Structure(StructureKind::Inserter),
        ItemKind::Structure(StructureKind::Chest),
        ItemKind::Structure(StructureKind::Belt),
        ItemKind::Structure(StructureKind::Splitter),
        ItemKind::Structure(StructureKind::Assembler),
    ];


    pub fn to_string(self) -> &'static str {
        match self {
            ItemKind::Coal => "coal",
            ItemKind::CopperOre => "copper_ore",
            ItemKind::IronOre => "iron_ore",
            ItemKind::Structure(StructureKind::Belt) => "belt",
            ItemKind::Structure(StructureKind::Splitter) => "splitter",
            ItemKind::Structure(StructureKind::Inserter) => "inserter",
            ItemKind::Structure(StructureKind::Chest) => "chest",
            ItemKind::Structure(StructureKind::Quarry) => "quarry",
            ItemKind::Structure(StructureKind::Assembler) => "assembler",
            ItemKind::Voxel(VoxelKind::Dirt) => "dirt_block",
            ItemKind::Voxel(VoxelKind::Stone) => "stone_block",

            ItemKind::IronPlate => "iron_plate",
            ItemKind::CopperPlate => "copper_plate",

            ItemKind::IronGearWheel => "iron_gear_wheel",
            ItemKind::IronRod => "iron_rod",
            ItemKind::CopperWire => "copper_wire",
            ItemKind::MechanicalComponent => "mechanical_component",
            ItemKind::ElectronicsKit => "electronics_kit",

            ItemKind::Voxel(_) => "invalid",
        }
    }


    pub fn max_stack_size(self) -> u32 {
        100
    }


    pub fn as_voxel(self) -> Option<VoxelKind> {
        match self {
            ItemKind::Voxel(vk) => Some(vk),
            _ => None,
        }
    }


    pub fn as_structure(self) -> Option<StructureKind> {
        match self {
            ItemKind::Structure(structure) => Some(structure),
            _ => None,
        }
    }


    ///
    /// This function returns a Mesh of the item centred at (0, 0, 0) with
    /// a unit size
    ///
    pub fn create_mesh(self) -> Mesh {
        match self {
            _ => {
                let colour = match self {
                    ItemKind::Voxel(vk) => vk.colour(),
                    ItemKind::Structure(structure) => return structure.mesh(),
                    ItemKind::CopperOre => VoxelKind::Copper.colour(),
                    ItemKind::IronOre => VoxelKind::Iron.colour(),
                    ItemKind::Coal => VoxelKind::Coal.colour(),
                    _ => Vec4::ONE,
                };

                let mut verticies = vec![];
                let mut indicies = vec![];

                let pos = Vec3::new(-0.5, -0.5, -0.5);
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Up, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Down, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Left, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Right, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Forward, pos, colour));
                draw_quad(&mut verticies, &mut indicies, Quad::from_direction(Direction::Back, pos, colour));

                Mesh::new(verticies, indicies)
            }
        }
    }
}


impl DroppedItem {
    pub fn new(item: Item, pos: Vec3) -> Self {
        DroppedItem {
            item,
            body: PhysicsBody {
                position: pos,
                velocity: (random::<Vec3>() - Vec3::ONE*0.5) * 5.0,
                aabb_dims: Vec3::splat(DROPPED_ITEM_SCALE),
            },

            creation_tick: Tick::NEVER,
        }
    }
}


impl Assets {
    pub fn new(texture_atlas: &mut TextureAtlasBuilder) -> Self {
        let textures_dir = Path::new("assets/textures");

        let mut textures = HashMap::with_capacity(ItemKind::ALL.len());
        let mut models = HashMap::with_capacity(ItemKind::ALL.len());

        let white_texture = texture_atlas.register(IVec2::new(1, 1), &[255, 255, 255, 255]);

        for &item in ItemKind::ALL {
            models.insert(item, item.create_mesh());

            let path = textures_dir.join(item.to_string()).with_added_extension("png");

            let texture = match File::open(&path) {
                Ok(buf) => {
                    let buf = BufReader::new(buf);
                    let asset = PngDecoder::new(buf).unwrap();
                    let dims = asset.dimensions();
                    let dims = IVec2::new(dims.0 as _, dims.1 as _);

                    let mut data = vec![0; asset.total_bytes() as _];
                    asset.read_image(&mut data).unwrap();

                    texture_atlas.register(dims, &data)
                }

                Err(_) => white_texture,
            };

            textures.insert(item, texture);
        }


        Self {
            models,
            textures,
        }
    }


    pub fn get(&self, kind: ItemKind) -> &Mesh {
        self.models.get(&kind).unwrap()
    }

    pub fn get_ico(&self, kind: ItemKind) -> TextureId {
        *self.textures.get(&kind).unwrap()
    }
}


impl core::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} x{}", self.kind, self.amount)
    }
}
