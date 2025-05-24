use std::{collections::BTreeMap, ops::Bound};

use glam::{IVec3, Mat4, Vec3};
use sti::{define_key, println};

use crate::{chunk::VoxelKind, gen_map::KeyGen, items::{Item, ItemKind, ItemMeshes}, mesh::Mesh, renderer::Renderer, shader::ShaderProgram, Game, World};

define_key!(pub StructureKey(u32));
define_key!(pub StructureGen(u32));


#[derive(Debug, PartialEq, Clone, Copy, Eq)]
pub struct StructureId(pub KeyGen<StructureGen, StructureKey>);


impl PartialOrd for StructureId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.key.partial_cmp(&other.0.key)
    }
}


impl Ord for StructureId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.key.cmp(&other.0.key)
    }
}


#[derive(Debug, Clone, Copy)]
pub enum CompassDirection {
    North,
    South,
    East,
    West,
}


impl CompassDirection {
    pub fn as_ivec3(self) -> IVec3 {
        match self {
            CompassDirection::North => IVec3::new(0, 0, 1),
            CompassDirection::South => IVec3::new(0, 0, -1),
            CompassDirection::East => IVec3::new(1, 0, 0),
            CompassDirection::West => IVec3::new(-1, 0, 0),
        }
    }
}


pub struct Structure {
    pub position: IVec3,
    pub direction: CompassDirection,
    pub data: StructureData,

    pub input : Option<Vec<Slot>>,
    pub output: Option<Slot>,

    pub is_queued: bool,
}


#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub item: Option<Item>,
    pub expected: Option<ItemKind>,
    pub max: u32,
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


pub struct WorkQueue {
    pub entries: BTreeMap<(u32, StructureId), ()>,
}


impl WorkQueue {
    pub fn new() -> Self { Self { entries: BTreeMap::new() } }


    pub fn insert(&mut self, tick: u32, id: StructureId) {
        self.entries.insert((tick, id), ());
    }


    pub fn process(&mut self, to_tick: u32) -> Vec<(u32, StructureId)> {
        let mut result = Vec::new();
        let mut cursor = self.entries.lower_bound_mut(Bound::Unbounded);
        while let Some(((tick, id), ())) = cursor.next() {
            if *tick > to_tick { break; }
            result.push((*tick, *id));
            cursor.remove_prev();
        }

        return result;
    }
}


impl Structure {
    pub fn from_kind(kind: StructureKind, pos: IVec3, direction: CompassDirection) -> Structure {
        match kind {
            StructureKind::Quarry => {
                Structure {
                    data: StructureData::from_kind(StructureKind::Quarry),
                    input: None,
                    output: Some(Slot { item: None, expected: None, max: 1 }),
                    position: pos,
                    direction,
                    is_queued: false,
                }
            },


            StructureKind::Inserter => {
                Structure {
                    position: pos,
                    direction,
                    data: StructureData::Inserter,
                    input: None,
                    output: None,
                    is_queued: false,
                }
            },
        }
    }


    pub fn zero_zero(&self) -> IVec3 {
        self.position - self.data.as_kind().origin(self.direction)
    }


    pub fn try_take(&mut self, work_queue: &mut WorkQueue) -> Option<Item> {
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


impl Slot {
    pub fn can_give(&self, item: Item) -> bool {
        if let Some(current_item) = self.item {
            if current_item.kind != item.kind { return false }
            if current_item.amount + item.amount > self.max {
                return false
            }

            return true;
        } else if let Some(expected) = self.expected {
            if expected != item.kind { return false }
            if item.amount > self.max { return false }
            return true;
        } else {
            return item.amount <= self.max;
        }
    }


    pub fn give(&mut self, item: Item) {
        assert!(self.can_give(item));
        if let Some(item) = &mut self.item {
            item.amount += item.amount;
        } else {
            self.item = Some(item);
        }
    }
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


macro_rules! blocks_arr {
    ($dir: expr, $($elem: expr),*) => {
        {
        const NORTH : &[IVec3] = &[$($elem),*];
        const SOUTH : &[IVec3] = &[$(rotate_block_vector(CompassDirection::South, $elem)),*];
        const EAST : &[IVec3] = &[$(rotate_block_vector(CompassDirection::East, $elem)),*];
        const WEST : &[IVec3] = &[$(rotate_block_vector(CompassDirection::West, $elem)),*];

        match $dir {
            CompassDirection::North => NORTH,
            CompassDirection::South => SOUTH,
            CompassDirection::East => EAST,
            CompassDirection::West => WEST,
        }
        }
        
    };
}



impl StructureKind {
    pub fn item_kind(self) -> ItemKind {
        ItemKind::Structure(self)
    }



    pub fn blocks(self, dir: CompassDirection) -> &'static [IVec3] {
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


    pub fn origin(self, dir: CompassDirection) -> IVec3 {
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


impl StructureId {
    pub fn update(self, game: &mut Game) {
        let structure = game.structures.get_mut(self.0).unwrap();
        let dir = structure.direction;
        let zz = structure.zero_zero();
        let output = structure.output;

        match &mut structure.data {
            StructureData::Quarry { current_progress } => {
                let x = *current_progress % 3;
                let z = (*current_progress / 3) % 3;
                let y = *current_progress / 9;
                
                let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
                let pos = rotate_block_vector(dir, pos);

                let item = game.block_item(zz + pos);
                let voxel = game.world.get_voxel(zz + pos);
                if !voxel.kind.is_air() {
                    if output.unwrap().can_give(item) {
                        game.break_block(zz + pos);
                    } else {
                        game.work_queue.insert(game.current_tick+12, self);
                        return;
                    }
                }

                let structure = game.structures.get_mut(self.0).unwrap();
                let StructureData::Quarry { current_progress } = &mut structure.data
                else { unreachable!() };

                structure.output.as_mut().unwrap().give(item);

                loop {
                    *current_progress += 1;

                    let x = *current_progress % 3;
                    let z = (*current_progress / 3) % 3;
                    let y = *current_progress / 9;
                    let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
                    let pos = rotate_block_vector(dir, pos);
                    let voxel = game.world.get_voxel(zz + pos);

                    if voxel.kind.is_air() { continue };

                    let mut hardness = voxel.kind.base_hardness();
                    if pos.y < 0 { 
                        hardness = (hardness as f32 * (1.0 + (pos.y as f32 * 0.01).powi(2))) as u32;
                    }

                    game.work_queue.insert(game.current_tick+hardness, self);
                    break;
                }

            },


            StructureData::Inserter => (),
        }
    }
}


impl Structure {
    pub fn render(&self, _: &Renderer, meshes: &ItemMeshes, shader: &ShaderProgram) {
        let kind = self.data.as_kind();

        let position = self.position - self.data.as_kind().origin(self.direction);
        let mesh = meshes.get(kind.item_kind());

        let blocks = self.data.as_kind().blocks(self.direction);
        let mut min = IVec3::MAX;
        let mut max = IVec3::MIN;
        for offset in blocks {
            min = min.min(position + offset);
            max = max.max(position + offset);
        }

        let mesh_position = (min + max).as_vec3() / 2.0 + Vec3::new(0.5, 0.0, 0.5);

        let rot = self.direction.as_ivec3().as_vec3();
        let rot = rot.x.atan2(rot.z);
        let rot = rot + 90f32.to_radians();
        let model = Mat4::from_translation(mesh_position) * Mat4::from_rotation_y(rot);
        shader.set_matrix4(c"model", model);

        mesh.draw();
    }


}


pub const fn rotate_block_vector(dir: CompassDirection, v: IVec3) -> IVec3 {
    match dir {
        CompassDirection::North => IVec3::new(v.x, v.y, v.z),
        CompassDirection::East  => IVec3::new(-v.z, v.y, v.x),
        CompassDirection::South => IVec3::new(-v.x, v.y, -v.z),
        CompassDirection::West  => IVec3::new(v.z, v.y, -v.x),
    }
}




pub fn rotate_vector(direction: Vec3, v: Vec3) -> IVec3 {
    let angle = direction.x.atan2(direction.z);
    let cos = angle.cos();
    let sin = angle.sin();

    let x = v.x as f32;
    let y = v.y as f32;
    let z = v.z as f32;

    let rotated_x = cos * x - sin * z;
    let rotated_z = sin * x + cos * z;

    IVec3::new(
        rotated_x.round() as i32,
        y.round() as i32,
        rotated_z.round() as i32,
    )
}



#[test]
fn test_work_queue() {
    let mut wq = WorkQueue { entries: BTreeMap::new() };

    let k1 = StructureId(KeyGen::new(StructureGen(0), StructureKey(1)));
    let k2 = StructureId(KeyGen::new(StructureGen(0), StructureKey(2)));
    let k3 = StructureId(KeyGen::new(StructureGen(0), StructureKey(3)));
    let k4 = StructureId(KeyGen::new(StructureGen(0), StructureKey(4)));

    wq.insert(10, k1);
    wq.insert(15, k2);
    wq.insert(20, k4);
    wq.insert(20, k3);

    assert_eq!(&*wq.process(9), &[]);
    assert_eq!(&*wq.process(10), &[(10, k1)]);
    assert_eq!(&*wq.process(17), &[(15, k2)]);
    assert_eq!(&*wq.process(25), &[(20, k3), (20, k4)]);
}



