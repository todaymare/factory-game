use std::{cell::Cell, num::{NonZeroU16, NonZeroU32}, rc::Rc};

use glam::{DVec3, IVec3, UVec3};
use rand::seq::IndexedRandom;
use sti::{define_key, vec::KVec};
use tracing::warn;
use wgpu::wgt::{DrawIndexedIndirectArgs, DrawIndirectArgs};

use crate::{constants::{CHUNK_SIZE_I32, QUAD_VERTICES, REGION_SIZE, RENDER_DISTANCE}, directions::Direction, frustum::Frustum, voxel_world::{chunker::{ChunkPos, RegionPos, WorldChunkPos}, mesh::{ChunkFaceMesh, ChunkMeshes}}};


#[derive(Debug)]
pub struct MeshOctree {
    pub nodes: KVec<u16, Node>,
    first_free: NodeId,
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(NonZeroU16);


#[derive(Debug)]
pub enum Node {
    Internal([NodeId; 8]),
    Leaf(Box<Leaf>),
}


#[derive(Debug)]
pub struct Leaf {
    pub mesh: [Option<ChunkFaceMesh>; 6]
}


impl NodeId {
    const INVALID : NodeId = NodeId(NonZeroU16::MAX);
}


impl Node {
    #[inline]
    fn internal(&self) -> &[NodeId; 8] {
        match self {
            Node::Internal(children) => children,
            Node::Leaf(_) => unreachable!()
        }
    }

    #[inline]
    fn internal_mut(&mut self) -> &mut [NodeId; 8] {
        match self {
            Node::Internal(children) => children,
            Node::Leaf(_) => unreachable!()
        }
    }

    #[inline]
    fn leaf(&self) -> &Leaf {
        match self {
            Node::Internal(_) => unreachable!(),
            Node::Leaf(mesh) => &mesh,
        }
    }

    #[inline]
    fn leaf_mut(&mut self) -> &mut Leaf {
        match self {
            Node::Internal(_) => unreachable!(),
            Node::Leaf(mesh) => mesh,
        }
    }
}


impl MeshOctree {
    const SIZE : usize = 32;
    const HEIGHT : u32 = Self::SIZE.ilog2();


    #[inline]
    fn child_idx(chunk_pos: ChunkPos, height: u32) -> usize {
        return
            (((chunk_pos.0.z >> height) as usize & 1) << 2) |
            (((chunk_pos.0.y >> height) as usize & 1) << 1) |
            (((chunk_pos.0.x >> height) as usize & 1) << 0);
    }


    #[inline]
    fn child_idx_to_delta(idx: usize, height: u32) -> ChunkPos {
        let idx = idx as u32;
        return
            ChunkPos(UVec3::new
            (((idx >> 0) & 1) << height,
             ((idx >> 1) & 1) << height,
             ((idx >> 2) & 1) << height));
    }


    pub fn new() -> MeshOctree {
        let mut nodes = KVec::new();
        nodes.push(Node::Internal([NodeId::INVALID; 8]));

        return MeshOctree {
            nodes,
            first_free: NodeId::INVALID,
        };
    }

    pub fn insert(&mut self, chunk_pos: ChunkPos, mesh: Leaf) -> NodeId {
        fn rec(this: &mut MeshOctree, chunk_pos: ChunkPos, at: u16, mut height: u32, mesh: Box<Leaf>) -> NodeId {
            height -= 1;

            let mut children = this.nodes[at].internal_mut();

            let child_idx = MeshOctree::child_idx(chunk_pos, height);

            if height > 0 {
                let mut child_id = children[child_idx];
                if child_id == NodeId::INVALID {
                    child_id = this.alloc(Node::Internal([NodeId::INVALID; 8]));
                    children = this.nodes[at].internal_mut();
                    children[child_idx] = child_id;
                }

                rec(this, chunk_pos, child_id.0.get(), height, mesh)
            }
            else {
                let child_id = children[child_idx];
                if child_id != NodeId::INVALID {
                    this.free(child_id);
                }

                let new_leaf = this.alloc(Node::Leaf(mesh));
                children = this.nodes[at].internal_mut();
                children[child_idx] = new_leaf;
                new_leaf
            }
        }


        rec(self, chunk_pos, 0, Self::HEIGHT, Box::new(mesh))
    }

    pub fn remove(&mut self, chunk_pos: ChunkPos) {
        fn rec(this: &mut MeshOctree, chunk_pos: ChunkPos, at: u16, mut height: u32) -> bool {
            height -= 1;

            let children = this.nodes[at].internal();

            let child_idx = MeshOctree::child_idx(chunk_pos, height);

            if height > 0 {
                let child_id = children[child_idx];
                if child_id == NodeId::INVALID {
                    let UVec3 { x, y, z } = chunk_pos.0;
                    println!("warn: octree node {x},{y},{z} did not exist");
                    return false;
                }

                let freed = rec(this, chunk_pos, child_id.0.get(), height);
                if freed {
                    let children = this.nodes[at].internal_mut();
                    children[child_idx] = NodeId::INVALID;

                    if let Some(non_root) = NonZeroU16::new(at) {
                        if *children == [NodeId::INVALID; 8] {
                            this.free(NodeId(non_root));
                            return true;
                        }
                    }
                }

                return false;
            }
            else {
                let child_id = children[child_idx];
                if child_id != NodeId::INVALID {
                    assert!(matches!(this.nodes[child_id.0.get()], Node::Leaf(_)));
                    this.free(child_id);
                    return true;
                }
                else {
                    let UVec3 { x, y, z } = chunk_pos.0;
                    println!("warn: octree node {x},{y},{z} did not exist");
                    return false;
                }
            }
        }

        rec(self, chunk_pos, 0, Self::HEIGHT);
    }


    pub fn render(
        &self, pos0: ChunkPos, region: RegionPos,
        player_chunk: WorldChunkPos, camera: DVec3,
        frustum: &Frustum, buffer: &mut Vec<DrawIndirectArgs>,
        remesh_buffer: &mut Vec<WorldChunkPos>, rd: i32,
        counter: &mut u32)
    {

        fn rec(
            this: &MeshOctree, pos0: ChunkPos, at: u16, height: u32,
            region: RegionPos, player_chunk: WorldChunkPos, camera: DVec3,
            frustum: &Frustum, buffer: &mut Vec<DrawIndirectArgs>,
            remesh_buffer: &mut Vec<WorldChunkPos>, rd: i32, counter: &mut u32,
        ) {

            let chunk_pos = (region.0 * REGION_SIZE as i32) + pos0.0.as_ivec3();

            let size = 2i32.pow(height);
            let min = chunk_pos * CHUNK_SIZE_I32;
            let max = (chunk_pos + IVec3::splat(size)) * CHUNK_SIZE_I32;


            let min = (min.as_dvec3() - camera).as_vec3();
            let max = (max.as_dvec3() - camera).as_vec3();

            let is_visible = frustum.is_box_visible(min, max);
            if !is_visible {
                return;
            }


            if height > 0 {
                let children = this.nodes[at].internal();
                for idx in 0..8 {
                    let child_id = children[idx];
                    if child_id != NodeId::INVALID {
                        let d = MeshOctree::child_idx_to_delta(idx, height - 1);
                        rec(
                            this, ChunkPos(pos0.0 + d.0), child_id.0.get(),
                            height - 1, region, player_chunk, camera,
                            frustum, buffer, remesh_buffer, rd, counter
                        );
                    }
                }
            }
            else {
                let leaf = this.nodes[at].leaf();
                let chunk_pos = (region.0 * REGION_SIZE as i32) + pos0.0.as_ivec3();
                let offset = chunk_pos - player_chunk.0;

                if offset.length_squared() > rd*rd {
                    return;
                }

                let min = chunk_pos * CHUNK_SIZE_I32;
                let max = (chunk_pos + IVec3::ONE) * CHUNK_SIZE_I32;
                
                let min = (min.as_dvec3() - camera).as_vec3();
                let max = (max.as_dvec3() - camera).as_vec3();

                if !frustum.is_box_visible(min, max) {
                    return;
                }


                let dir_from_camera = offset.as_vec3().normalize();

                for (i, mesh) in leaf.mesh.iter().enumerate() {
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


                    let vo = mesh.quads.offset as u32;
                    let vs = mesh.quads.size as u32;

                    *counter += vs * 6;
                    buffer.push(DrawIndirectArgs {
                        instance_count: vs,
                        first_instance: vo,
                        vertex_count: 6,
                        first_vertex: 0,
                    });

                }

            }
        }


        rec(
            self, pos0, 0, Self::HEIGHT, region,
            player_chunk, camera, frustum, buffer, remesh_buffer, rd, counter
        );
    }


    fn alloc(&mut self, node: Node) -> NodeId {
        if self.first_free != NodeId::INVALID {
            let node_id = self.first_free;
            self.first_free = self.nodes[node_id.0.get()].internal()[0];
            self.nodes[node_id.0.get()] = node;
            return node_id;
        }
        else {
            let idx = self.nodes.push(node);
            return NodeId(NonZeroU16::new(idx).unwrap());
        }
    }

    fn free(&mut self, node_id: NodeId) {
        assert!(node_id.0.get() != 0);

        let mut node = [NodeId::INVALID; 8];
        node[0] = self.first_free;
        self.nodes[node_id.0.get()] = Node::Internal(node);
        self.first_free = node_id;
    }


    pub fn get(&self, node: NodeId) -> &Leaf {
        self.nodes[node.0.get()].leaf()
    }


    pub fn get_mut(&mut self, node: NodeId) -> &mut Leaf {
        self.nodes[node.0.get()].leaf_mut()
    }

}


