use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use glam::{DVec3, IVec2, IVec3, Vec3, Vec4};
use image::{codecs::png::PngDecoder, EncodableLayout, ImageDecoder};
use rand::random;
use tracing::error;

use crate::{directions::Direction, mesh::{self, Mesh}, quad::Quad, renderer::textures::{TextureAtlasBuilder, TextureId}, structures::strct::StructureKind, voxel_world::voxel::Voxel, PhysicsBody, Tick, DROPPED_ITEM_SCALE};


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
    Voxel(Voxel),

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
    pub cube: Mesh,
}


impl Item {
    pub const fn new(kind: ItemKind, amount: u32) -> Self {
        Self {
            amount,
            kind,
        }
    }


    pub fn with_amount(self, amount: u32) -> Item {
        Item::new(self.kind, amount)
    }
}


impl ItemKind {
    pub const ALL : &[ItemKind] = &[
        ItemKind::Voxel(Voxel::Dirt),
        ItemKind::Voxel(Voxel::Stone),
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
        ItemKind::Structure(StructureKind::Silo),
        ItemKind::Structure(StructureKind::Belt),
        ItemKind::Structure(StructureKind::Splitter),
        ItemKind::Structure(StructureKind::Assembler),
        ItemKind::Structure(StructureKind::Furnace),
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
            ItemKind::Structure(StructureKind::Silo) => "silo",
            ItemKind::Structure(StructureKind::Quarry) => "quarry",
            ItemKind::Structure(StructureKind::Assembler) => "assembler",
            ItemKind::Structure(StructureKind::Furnace) => "furnace",
            ItemKind::Voxel(Voxel::Dirt) => "dirt_block",
            ItemKind::Voxel(Voxel::Stone) => "stone_block",

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


    pub fn as_voxel(self) -> Option<Voxel> {
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
}


impl DroppedItem {
    pub fn new(item: Item, pos: DVec3) -> Self {
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
        let white_mesh = {
            let data = &[u32::MAX];
            let mut vertices = vec![];
            let mut indices = vec![];

            voxel_mesher::greedy_mesh(&*data, IVec3::new(1, 1, 1), &mut vertices, &mut indices, Vec3::ONE);
            Mesh::new(&vertices, &indices)
        };

        for &item in ItemKind::ALL {
            // load texture
            let path = textures_dir.join(item.to_string()).with_added_extension("png");

            let texture = match File::open(&path) {
                Ok(buf) => {
                    let buf = BufReader::new(buf);
                    let asset = PngDecoder::new(buf).unwrap();
                    let dims = asset.dimensions();
                    let dims = IVec2::new(dims.0 as _, dims.1 as _);

                    let mut data = vec![0; asset.total_bytes() as usize];
                    asset.read_image(&mut data).unwrap();

                    let id = texture_atlas.register(dims, &data);

                    if let ItemKind::Structure(kind) = item {
                        models.insert(item, kind.mesh());
                    } else {
                        let mut vertices = vec![];
                        let mut indices = vec![];
                        let data = {
                            let mut vec = Vec::with_capacity(data.len() / 4);
                            for mut bytes in data.iter().copied().array_chunks::<4>() {
                                bytes.reverse();
                                vec.push(u32::from_ne_bytes(bytes));
                            }

                            vec.reverse();
                            vec
                        };

                        voxel_mesher::greedy_mesh(&data, IVec3::new(dims.x, dims.y, 1), &mut vertices, &mut indices, 1.0/Vec3::new(dims.x as _, dims.y as _, 8.0));
                        let mesh = Mesh::new(&vertices, &indices);
                        models.insert(item, mesh);
                    }

                    id
                }

                Err(_) => {
                    error!("unable to find a texture for '{}'", item.to_string());
                    models.insert(item, white_mesh.clone());
                    white_texture
                },
            };

            textures.insert(item, texture);


            // create mesh

        }


        Self {
            models,
            textures,
            cube: white_mesh,
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
