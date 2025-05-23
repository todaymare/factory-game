use std::{collections::BTreeMap, ops::Bound};

use glam::{IVec3, Mat4, Vec3};
use sti::{define_key, println};

use crate::{chunk::VoxelKind, gen_map::KeyGen, items::{ItemKind, ItemMeshes}, mesh::Mesh, renderer::Renderer, shader::ShaderProgram, Game, World};

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


pub struct Structure {
    pub position: IVec3,
    pub direction: CompassDirection,
    pub data: StructureData,
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
    pub fn quarry(pos: IVec3, compass_direction: CompassDirection) -> Structure {
        Structure { data: StructureData::from_kind(StructureKind::Quarry), position: pos, direction: compass_direction }
    }


    pub fn blueprint_origin() {
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


impl StructureKind {
    pub fn item_kind(self) -> ItemKind {
        ItemKind::Structure(self)
    }



    pub fn blocks(self) -> &'static [IVec3] {
        match self {
            StructureKind::Quarry => {
                const BLOCKS : &[IVec3] = &[
                    IVec3::new(0, 0, 0), IVec3::new(1, 0, 0),
                    IVec3::new(2, 0, 0), IVec3::new(3, 0, 0),
                    IVec3::new(4, 0, 0),

                    IVec3::new(0, 0, 1), IVec3::new(4, 0, 1),
                    IVec3::new(0, 0, 2), IVec3::new(4, 0, 2),
                    IVec3::new(0, 0, 3), IVec3::new(4, 0, 3),

                    IVec3::new(0, 0, 4), IVec3::new(1, 0, 4),
                    IVec3::new(2, 0, 4), IVec3::new(3, 0, 4),
                    IVec3::new(4, 0, 4),
                ];
                BLOCKS
            },

            StructureKind::Inserter => {
                const BLOCKS : &[IVec3] = &[
                    IVec3::new(0, 0, 0),
                    IVec3::new(0, 0, 1), 
                    IVec3::new(0, 0, 2),
                ];

                BLOCKS
            }
        }
    }


    pub fn origin(self) -> IVec3 {
        match self {
            StructureKind::Quarry => IVec3::new(2, 0, 4),
            StructureKind::Inserter => IVec3::new(0, 0, 0),
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
        let origin = structure.position;

        match &mut structure.data {
            StructureData::Quarry { current_progress } => {
                let x = *current_progress % 3;
                let z = (*current_progress / 3) % 3;
                let y = *current_progress / 9;
                
                let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);

                let voxel = game.world.get_voxel(structure.position + pos);
                if !voxel.kind.is_air() {
                    game.break_block(origin + pos);
                }

                let structure = game.structures.get_mut(self.0).unwrap();
                let StructureData::Quarry { current_progress } = &mut structure.data
                else { unreachable!() };

                loop {
                    *current_progress += 1;

                    let x = *current_progress % 3;
                    let z = (*current_progress / 3) % 3;
                    let y = *current_progress / 9;
                    let pos = IVec3::new(x as i32 + 1, -(y as i32) - 1, z as i32 + 1);
                    let voxel = game.world.get_voxel(structure.position + pos);

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

        let position = self.position;
        let mesh = meshes.get(kind.item_kind());

        let model = Mat4::from_translation(position.as_vec3());
        shader.set_matrix4(c"model", model);

        mesh.draw();
    }
}


pub fn rotate_block_vector(direction: Vec3, v: IVec3) -> IVec3 {
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



