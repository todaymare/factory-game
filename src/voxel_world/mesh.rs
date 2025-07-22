use std::{cell::Cell, mem::offset_of, num::NonZeroU32, ptr::null_mut, rc::Rc, sync::Arc};

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, UVec3, UVec4, Vec3, Vec4};
use sti::{define_key, key::Key};
use wgpu::{util::{DeviceExt, StagingBelt}, ShaderStages};

use crate::{buddy_allocator::BuddyAllocator, constants::{CHUNK_SIZE, CHUNK_SIZE_I32}, directions::Direction, octree::NodeId, renderer::{gpu_allocator::{GPUAllocator, GpuPointer}, uniform::Uniform}};

use super::{chunk::ChunkData, voxel::Voxel};

#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
#[repr(C)]
pub struct ChunkQuadInstance {
    // pos.x : 0..6
    // pos.y : 6..12
    // pos.z : 12..18
    // width : 18..23
    // height: 23..28
    // empty : 28..32
    p1: u32,
    // texture id: 0..8
    // vertex 1 ao: 8..10
    // vertex 2 ao: 10..12
    // vertex 3 ao: 12..14
    // vertex 4 ao: 14..16
    // debug is_chunk_loaded: 16..17
    id: u32,

    chunk_index: u32,
}


define_key!(pub VoxelMeshIndex(u32));


#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
#[repr(C)]
pub struct ChunkMeshFramedata {
    pub offset: IVec3,
    pub normal: u32, // needs to be 4 bytes anyways cos we need to align to 16 bytes
}


#[derive(Debug)]
pub struct ChunkFaceMesh {
    pub quads: GpuPointer<ChunkQuadInstance>,
    pub index_count: u32,
    pub chunk_mesh_data_index: VoxelMeshIndex, 
}


#[derive(Debug)]
pub struct ChunkMeshes {
    pub meshes: Option<NodeId>,
    pub version: NonZeroU32,
}


pub struct ChunkDataRef {
    chunks: [Option<Arc<ChunkData>>; 27],
}


impl<'a> ChunkDataRef {
    pub fn new(chunks: [Option<Arc<ChunkData>>; 27]) -> Self {
        Self {
            chunks,
        }
    }


    pub fn get(&self, mut pos: IVec3) -> Voxel {
        pos += CHUNK_SIZE_I32;

        let chunk = pos / CHUNK_SIZE_I32;

        let chunk_idx =
              9*chunk.x
            + 3*chunk.y
            + 1*chunk.z;

        self.chunks[chunk_idx as usize]
            .as_ref()
            .map(|x| x.get(pos & (CHUNK_SIZE_I32-1)))
            .unwrap_or(Voxel::Air)
    }

    pub fn is_neighbour(&self, mut pos: IVec3) -> bool {
        pos += CHUNK_SIZE_I32;

        let chunk = pos / CHUNK_SIZE_I32;

        let chunk_idx =
              9*chunk.x
            + 3*chunk.y
            + 1*chunk.z;
        chunk_idx == 5
    }
}


impl ChunkFaceMesh {
    pub fn new(
        belt: &mut StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        vertex_allocator: &mut GPUAllocator<ChunkQuadInstance>,

        vertices: &[ChunkQuadInstance], 
        index: VoxelMeshIndex,
   ) -> Self {
        debug_assert!(vertices.iter().all(|x| x.chunk_index == index.usize() as u32));

        let vertex = vertex_allocator.allocate_slice(belt, encoder, device, vertices);

        Self { quads: vertex, index_count: vertices.len() as u32, chunk_mesh_data_index: index }
    }
}


impl ChunkQuadInstance {
    pub fn new(pos: IVec3, ty: Voxel, h: u32, l: u32, normal: u8, ao: u32, chunk_index: VoxelMeshIndex) -> Self {
        let UVec3 { x, y, z } = pos.as_uvec3();

        debug_assert!(x <= 32 && y <= 32 && z <= 32, "{x} {y} {z} {l}x{h} {normal}");
        debug_assert!(h-1 < 32);
        debug_assert!(l-1 < 32);

        let base = 
            ( (x      & 0x3F) as u32)                |  // 6 bits
            (((y      & 0x3F) as u32) <<  6)         |  // 6 bits
            (((z      & 0x3F) as u32) << 12)         |  // 6 bits
            (((l-1    & 0x1F) as u32) << 18)         |  // 5 bits
            (((h-1    & 0x1F) as u32) << 23)         ;  // 5 bits


        let id = ty.texture_id(Direction::from_normal(normal));
        debug_assert!(id < 256);
        debug_assert_eq!(ao, ao & 0x1FF);

        let id = (ao << 8)
                 | id;

        Self { chunk_index: chunk_index.usize() as u32, p1: base, id }
    }


    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] =
            &wgpu::vertex_attr_array![2 => Uint32, 3 => Uint32, 4 => Uint32];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ChunkQuadInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}


