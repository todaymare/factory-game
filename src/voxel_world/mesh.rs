use std::{mem::offset_of, num::NonZeroU32, ptr::null_mut};

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, UVec3, UVec4, Vec3, Vec4};
use sti::key::Key;
use wgpu::{util::{DeviceExt, StagingBelt}, ShaderStages};

use crate::{buddy_allocator::BuddyAllocator, octree::NodeId, renderer::{gpu_allocator::{GPUAllocator, GpuPointer}, uniform::Uniform, MeshIndex}};

#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
#[repr(C)]
pub struct ChunkQuadInstance {
    // pos.x : 0..6
    // pos.y : 6..12
    // pos.z : 12..18
    // width : 18..23
    // height: 23..28
    // red   : 28..36
    // green : 36..44
    // blue  : 44..52
    // empty : 52..64
    p1: u32,
    p2: u32,

    chunk_index: u32,
}


#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
#[repr(C)]
pub struct ChunkMeshFramedata {
    pub offset: IVec3,
    pub normal: u32, // needs to be 4 bytes anyways cos we need to align to 16 bytes
}


#[derive(Debug)]
pub struct ChunkFaceMesh {
    pub vertex: GpuPointer<ChunkQuadInstance>,
    pub index_count: u32,
    pub chunk_mesh_data_index: MeshIndex, 
}


pub struct ChunkMeshes {
    pub meshes: Option<NodeId>,
    pub version: NonZeroU32,
}


impl ChunkFaceMesh {
    pub fn new(
        belt: &mut StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        vertex_allocator: &mut GPUAllocator<ChunkQuadInstance>,

        vertices: &[ChunkQuadInstance], 
        index: MeshIndex,
   ) -> Self {
        debug_assert!(vertices.iter().all(|x| x.chunk_index == index.usize() as u32));

        let vertex = vertex_allocator.allocate_slice(belt, encoder, device, vertices);

        Self { vertex, index_count: vertices.len() as u32, chunk_mesh_data_index: index }
    }
}


impl ChunkQuadInstance {
    pub fn new(pos: IVec3, colour: Vec4, h: u32, l: u32, normal: u8, chunk_index: MeshIndex) -> Self {
        let UVec3 { x, y, z } = pos.as_uvec3();
        let UVec4 { x: r, y: g, z: b, w: _ } = (colour * 255.0).as_uvec4();

        debug_assert!(x <= 32 && y <= 32 && z <= 32, "{x} {y} {z} {l}x{h} {normal}");
        debug_assert!(h-1 < 32);
        debug_assert!(l-1 < 32);

        let base = 
            ( (x      & 0x3F)         as u64)        |  // 6 bits
            (((y      & 0x3F) as u64) <<  6)         |  // 6 bits
            (((z      & 0x3F) as u64) << 12)         |  // 6 bits
            (((l-1    & 0x1F) as u64) << 18)         |  // 5 bits
            (((h-1    & 0x1F) as u64) << 23)         |  // 5 bits
            (((r      & 0xFF) as u64) << 28)         |  // 8 bits
            (((g      & 0xFF) as u64) << 36)         |  // 8 bits
            (((b      & 0xFF) as u64) << 44);


        let b = base.to_le_bytes();
        let p1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let p2 = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);

        Self { chunk_index: chunk_index.usize() as u32, p1, p2 }
    }


    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] =
            &wgpu::vertex_attr_array![1 => Uint32, 2 => Uint32, 3 => Uint32];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ChunkQuadInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}


