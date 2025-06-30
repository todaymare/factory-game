use glam::{DVec3, IVec3, UVec3, Vec4};
use sti::{define_key, key::Key, vec::KVec};
use tracing::warn;
use wgpu::wgt::DrawIndexedIndirectArgs;

use crate::{constants::{CHUNK_SIZE_I32, REGION_SIZE}, directions::Direction, free_list::FreeKVec, frustum::Frustum, renderer::ChunkIndex, voxel_world::mesh::ChunkMesh, QUAD_INDICES, RENDER_DISTANCE};


#[derive(Debug)]
pub struct Octree {
    pub root: NodeId,
    pub nodes: KVec<NodeId, Node>,
    free: Vec<NodeId>,
}


define_key!(pub NodeId(u32));


#[derive(Debug)]
pub enum Node {
    Branch([Option<NodeId>; 8]),
    Leaf(UVec3, Box<[Option<ChunkMesh>; 6]>),
}


const MAX_DEPTH : u32 = REGION_SIZE.ilog2();


impl Octree {
    pub fn new() -> Self {
        let mut nodes = KVec::new();
        let root = nodes.push(Node::Branch([None; 8]));

        Self {
            root,
            nodes,
            free: vec![],
        }
    }


    pub fn orphan(&mut self, pos: UVec3, data: [Option<ChunkMesh>; 6]) -> NodeId {
        let node = Node::Leaf(pos, Box::new(data));
        self._orphan(node)
    }


    pub fn _orphan(&mut self, node: Node) -> NodeId {
        if let Some(pop) = self.free.pop() {
            self.nodes[pop] = node;
            pop
        } else {
            self.nodes.push(node)
        }
    }


    pub fn get(&self, node: NodeId) -> &[Option<ChunkMesh>; 6] {
        match &self.nodes[node] {
            Node::Branch(_) => unreachable!(),
            Node::Leaf(_, data) => data,
        }
    }


    pub fn get_mut(&mut self, node: NodeId) -> &mut [Option<ChunkMesh>; 6] {
        match &mut self.nodes[node] {
            Node::Branch(_) => unreachable!(),
            Node::Leaf(_, data) => data,
        }
    }


    pub fn insert(&mut self, pos: UVec3, data: [Option<ChunkMesh>; 6]) -> NodeId {
        let node = self.orphan(pos, data);
        self._insert(self.root, 0, pos, node);
        node
    }


    pub fn find(&mut self, pos: UVec3) -> Option<NodeId> {
        self._find(self.root, 0, pos)
    }


    pub fn remove(&mut self, pos: UVec3) {
        let result = self._remove(self.root, 0, pos);
        assert!(!result);
    }


    fn _insert(&mut self, curr_id: NodeId, depth: u32, pos: UVec3, node_id: NodeId) -> Option<NodeId> {
        if depth > MAX_DEPTH {
            warn!("exceeded max depth with pos {pos}. is it within bounds?");
            return None
        }
        let root = &mut self.nodes[curr_id];

        match root {
            Node::Branch(nodes) => {
                let index = which_child_is_this_position_in(pos, depth);
                if let Some(parent) = nodes[index] {
                    if let Some(node) = self._insert(parent, depth+1, pos, node_id) {
                        let Node::Branch(nodes) = &mut self.nodes[curr_id]
                        else { unreachable!() };
                        nodes[index] = Some(node);
                    }
                    return None;
                } 

                nodes[index] = Some(node_id);
                return None;
            },

            Node::Leaf(leaf_pos, _) => {
                let leaf_pos = *leaf_pos;

                let new_node = self._orphan(Node::Branch([None; 8]));

                self._insert(new_node, depth, leaf_pos, curr_id);
                self._insert(new_node, depth, pos, node_id);
                return Some(new_node)
            },
        }

    }


    fn _remove(&mut self, parent_id: NodeId, depth: u32, pos: UVec3) -> bool {
        let root = &self.nodes[parent_id];

        match root {
            Node::Branch(nodes) => {
                let index = which_child_is_this_position_in(pos, depth);
                if let Some(parent) = nodes[index] {
                    if self._remove(parent, depth+1, pos) {
                        let Node::Branch(nodes) = &mut self.nodes[parent_id]
                        else { unreachable!() };
                        nodes[index] = None;
                        self.free.push(parent);
                    }
                } 

                return false;
            },

            Node::Leaf(_, _) => {
                return true
            },
        }
    }


    fn _find(&self, parent_id: NodeId, depth: u32, pos: UVec3) -> Option<NodeId> {
        let root = &self.nodes[parent_id];

        match root {
            Node::Branch(nodes) => {
                let index = which_child_is_this_position_in(pos, depth);
                if let Some(parent) = nodes[index] {
                    return self._find(parent, depth+1, pos);
                } 

                return None;
            },

            Node::Leaf(_, _) => {
                return Some(parent_id)
            },
        }
    }


    pub fn render(
        &self,
        parent_id: NodeId,
        depth: u32,
        region: IVec3,
        curr_pos: UVec3,
        player_chunk: IVec3,
        buffer: &mut Vec<DrawIndexedIndirectArgs>,
        chunk_offsets: &mut FreeKVec<ChunkIndex, Vec4>,
        frustum: &Frustum,
        camera: DVec3,
        counter: &mut usize,
        rendered_counter: &mut usize,

    ) {

        *counter += 1;
        let root = &self.nodes[parent_id];

        match root {
            Node::Branch(nodes) => {
                let chunk_pos = (region * REGION_SIZE as i32) + curr_pos.as_ivec3();

                let size = 2i32.pow(MAX_DEPTH - depth);
                let min = chunk_pos * CHUNK_SIZE_I32;
                let max = (chunk_pos + IVec3::splat(size)) * CHUNK_SIZE_I32;


                let min = (min.as_dvec3() - camera).as_vec3();
                let max = (max.as_dvec3() - camera).as_vec3();

                let child_size = 2u32.pow(MAX_DEPTH-depth-1);
                let is_visible = frustum.is_box_visible(min, max);

                if !is_visible {
                    return;
                }
                
                for (i, node) in nodes.iter().enumerate() {
                    let i = i as u32;
                    let child_offset = UVec3::new(
                        (i >> 2) & 1,
                        (i >> 1) & 1,
                        i & 1,
                    );

                    let depth = depth + 1;
                    let pos = curr_pos + child_offset * child_size;

                    if let Some(node) = node {
                        self.render(
                            *node,
                            depth,
                            region,
                            pos,
                            player_chunk,
                            buffer,
                            chunk_offsets,
                            frustum,
                            camera,
                            counter,
                            rendered_counter,
                        );
                    }
                } 

            },

            Node::Leaf(v, meshes) => {
                let chunk_pos = (region * REGION_SIZE as i32) + v.as_ivec3();
                let offset = chunk_pos - player_chunk;

                if offset.length_squared() > RENDER_DISTANCE*RENDER_DISTANCE {
                    return;
                }

                let min = chunk_pos * CHUNK_SIZE_I32;
                let max = (chunk_pos + IVec3::ONE) * CHUNK_SIZE_I32;
                
                let min = (min.as_dvec3() - camera).as_vec3();
                let max = (max.as_dvec3() - camera).as_vec3();

                if !frustum.is_box_visible(min, max) {
                    return;
                }

                *rendered_counter += 1;

                let dir_from_camera = offset.as_vec3().normalize();

                let offset = chunk_pos * IVec3::splat(CHUNK_SIZE_I32);
                let offset = offset.as_dvec3() - camera;
                let offset = offset.as_vec3();


                for (i, mesh) in meshes.iter().enumerate() {
                    let Some(mesh) = mesh
                    else { continue };

                    if mesh.index_count == 0 {
                        warn!("an empty mesh was generated");
                        continue;
                    }

                    let normal = Direction::NORMALS[i];
                    if dir_from_camera.dot(normal) > 0.0 {
                        continue
                    }


                    let vo = mesh.vertex.offset as u32;
                    let vs = mesh.vertex.size as u32;

                    buffer.push(DrawIndexedIndirectArgs {
                        index_count: QUAD_INDICES.len() as _,
                        instance_count: vs,
                        first_index: 0,
                        base_vertex: 0,
                        first_instance: vo,
                    });

                    *chunk_offsets.get_mut(mesh.chunk_mesh_data_index) = Vec4::new(offset.x, offset.y, offset.z, 0.0);
                }


            },
        } 
    }
}


fn which_child_is_this_position_in(pos: UVec3, depth: u32) -> usize {
    let shift = MAX_DEPTH - depth - 1;
    let x = (pos.x >> shift) & 1;
    let y = (pos.y >> shift) & 1;
    let z = (pos.z >> shift) & 1;

    ((x << 2) | (y << 1) | z) as usize
}


