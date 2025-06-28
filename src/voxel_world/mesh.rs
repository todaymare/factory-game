use std::{mem::offset_of, ptr::null_mut};

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, UVec3, UVec4, Vec3, Vec4};
use sti::key::Key;
use wgpu::{util::{DeviceExt, StagingBelt}, ShaderStages};

use crate::{buddy_allocator::BuddyAllocator, renderer::{gpu_allocator::{GPUAllocator, GpuPointer}, uniform::Uniform, ChunkIndex}};

#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
#[repr(C)]
pub struct ChunkQuadInstance {
    data1: u32,
    data2: u32,
    w: u32,
    h: u32,
    chunk_index: u32,
}


#[derive(Debug)]
pub struct ChunkMesh {
    pub vertex: GpuPointer<ChunkQuadInstance>,
    pub index_count: u32,
    pub chunk_mesh_data_index: ChunkIndex, 
}


impl ChunkMesh {
    pub fn new(
        belt: &mut StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        vertex_allocator: &mut GPUAllocator<ChunkQuadInstance>,

        vertices: &[ChunkQuadInstance], 
        index: ChunkIndex,
   ) -> Self {
        debug_assert!(vertices.iter().all(|x| x.chunk_index == index.usize() as u32));

        let vertex = vertex_allocator.allocate_slice(belt, encoder, device, vertices);

        Self { vertex, index_count: vertices.len() as u32, chunk_mesh_data_index: index }
    }
}


impl ChunkQuadInstance {
    pub fn new(pos: IVec3, colour: Vec4, h: u32, l: u32, normal: u8, chunk_index: ChunkIndex) -> Self {
        let UVec3 { x, y, z } = pos.as_uvec3();
        let UVec4 { x: r, y: g, z: b, w: a } = (colour * 255.0).as_uvec4();

        debug_assert!(x <= 32 && y <= 32 && z <= 32, "{x} {y} {z}");
        debug_assert!(h <= 32);
        debug_assert!(l <= 32);
        debug_assert!(normal < 6);
        let pos = ((z as u32) << 12) | ((y as u32) << 6) | (x as u32);
        let pos = pos << 3 | normal as u32;
        let colour = ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32);

        let data1 = pos as u32;
        let data2 = colour as u32;

        Self { data1, data2, w: l, h, chunk_index: chunk_index.usize() as u32 }
    }


    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] =
            &wgpu::vertex_attr_array![1 => Uint32, 2 => Uint32, 3 => Uint32, 4 => Uint32, 5 => Uint32];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ChunkQuadInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}


#[derive(Debug)]
///! plane data with 4 vertices
pub struct Quad {
    pub color: Vec4,
    pub corners: [IVec3; 4],
    pub normal: u8,
}

/*
impl Quad {
    // the input position is assumed to be a voxel's (0,0,0) pos
    // therefore right / up / forward are offset by 1
    #[inline]
    pub fn from_direction(direction: Direction, pos: Vec3, color: Vec4) -> Self {
        let corners = match direction {
            Direction::Left => [
                Vec3::new(pos.x+1.0, pos.y, pos.z),
                Vec3::new(pos.x+1.0, pos.y, pos.z + 1.0),
                Vec3::new(pos.x+1.0, pos.y + 1.0, pos.z + 1.0),
                Vec3::new(pos.x+1.0, pos.y + 1.0, pos.z),
            ],
            Direction::Right => [
                Vec3::new(pos.x, pos.y + 1.0, pos.z),
                Vec3::new(pos.x, pos.y + 1.0, pos.z + 1.0),
                Vec3::new(pos.x, pos.y, pos.z + 1.0),
                Vec3::new(pos.x, pos.y, pos.z),
            ],
            Direction::Down => [
                Vec3::new(pos.x, pos.y, pos.z + 1.0),
                Vec3::new(pos.x + 1.0, pos.y, pos.z + 1.0),
                Vec3::new(pos.x + 1.0, pos.y, pos.z),
                Vec3::new(pos.x, pos.y, pos.z),
            ],
            Direction::Up => [
                Vec3::new(pos.x    , pos.y+1.0, pos.z),
                Vec3::new(pos.x + 1.0, pos.y+1.0, pos.z),
                Vec3::new(pos.x + 1.0, pos.y+1.0, pos.z + 1.0),
                Vec3::new(pos.x,   pos.y+1.0, pos.z + 1.0),
            ],
            Direction::Back => [
                Vec3::new(pos.x + 1.0, pos.y, pos.z),
                Vec3::new(pos.x + 1.0, pos.y + 1.0, pos.z),
                Vec3::new(pos.x, pos.y + 1.0, pos.z),
                Vec3::new(pos.x, pos.y, pos.z),
            ],
            Direction::Forward => [
                Vec3::new(pos.x, pos.y, pos.z+1.0),
                Vec3::new(pos.x, pos.y + 1.0, pos.z+1.0),
                Vec3::new(pos.x + 1.0, pos.y + 1.0, pos.z+1.0),
                Vec3::new(pos.x + 1.0, pos.y, pos.z+1.0),
            ],
        };

        Self {
            corners,
            color,
            normal: 
        }
    }

}*/

