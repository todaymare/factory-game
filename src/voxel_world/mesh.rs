use std::{mem::offset_of, ptr::null_mut};

use bytemuck::{Pod, Zeroable};
use glam::{UVec3, UVec4, Vec3, Vec4};
use wgpu::{util::{DeviceExt, StagingBelt}, ShaderStages};

use crate::{buddy_allocator::BuddyAllocator, renderer::{gpu_allocator::{GPUAllocator, GpuPointer}, uniform::Uniform}};

#[derive(Debug, Clone, Copy, Pod, Zeroable, PartialEq)]
#[repr(C)]
pub struct ChunkVertex {
    data1: u32,
    data2: u32,
}

const QUAD_VERTICES: &[f32] = &[
    // positions     // colors
    -1.0,  1.0,
     1.0, -1.0,
    -1.0, -1.0,

    -1.0,  1.0,
     1.0, -1.0,
     1.0,  1.0,
];


#[derive(Debug)]
pub struct ChunkMesh {
    pub vertex: GpuPointer<ChunkVertex>,
    pub index: GpuPointer<u32>,
    pub index_count: u32,
}


impl ChunkMesh {
    pub fn new(
        belt: &mut StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        vertex_allocator: &mut GPUAllocator<ChunkVertex>,
        index_allocator: &mut GPUAllocator<u32>,

        vertices: &[ChunkVertex], 
        indices: &[u32],
   ) -> Self {
        let vertex = vertex_allocator.allocate_slice(belt, encoder, device, vertices);

        let index = index_allocator.allocate_slice(belt, encoder, device, &indices);


        Self { vertex, index, index_count: indices.len() as u32 }
    }
}


impl ChunkVertex {
    pub fn new(pos: Vec3, colour: Vec4, normal: u8) -> Self {
        let UVec3 { x, y, z } = pos.as_uvec3();
        let UVec4 { x: r, y: g, z: b, w: a } = (colour * 255.0).as_uvec4();

        debug_assert!(x <= 32 && y <= 32 && z <= 32, "{x} {y} {z}");
        debug_assert!(normal < 6);
        let pos = ((z as u32) << 12) | ((y as u32) << 6) | (x as u32);
        let pos = pos << 3 | normal as u32;
        let colour = ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32);

        let data1 = pos as u32;
        let data2 = colour as u32;

        Self { data1, data2 }
    }


    pub fn set_colour(&mut self, colour: Vec4) {
        let UVec4 { x: r, y: g, z: b, w: a } = (colour * 255.0).as_uvec4();
        let colour = ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32);
        self.data2 = colour;
    }


    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] =
            &wgpu::vertex_attr_array![0 => Uint32, 1 => Uint32];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ChunkVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}


#[derive(Debug)]
///! plane data with 4 vertices
pub struct Quad {
    pub color: Vec4,
    pub corners: [Vec3; 4],
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


pub fn draw_quad(verticies: &mut Vec<ChunkVertex>, indicies: &mut Vec<u32>, quad: Quad) {
    let k = verticies.len() as u32;
    let mut i = 0;
    for corner in quad.corners {
        let mut colour = quad.color;
        colour = colour * 0.9 + colour * (i as f32 * 0.1);
        colour.w = quad.color.w;
        verticies.push(ChunkVertex::new(Vec3::new(corner[0] as f32, corner[1] as f32, corner[2] as f32), colour, quad.normal));
        i += 1;
    }


    indicies.extend_from_slice(&[k, k+1, k+2, k+2, k+3, k]);
}
