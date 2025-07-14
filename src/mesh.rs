use std::{io::{Read, Seek}, mem::offset_of, ptr::null_mut};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use tracing::warn;
use voxel_mesher::VoxelMesh;
use wgpu::{util::{BufferInitDescriptor, DeviceExt}, Buffer, BufferUsages};


#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct MeshInstance {
    modulate: Vec4,
    model: Mat4,
}


#[derive(Debug)]
pub struct Mesh {
    pub vertices: Buffer,
    pub indices : Buffer,
}


impl Mesh {
    pub fn from_vmf(device: &wgpu::Device, path: &str) -> Mesh {
        if !path.ends_with(".vmf") {
            warn!("mesh path should have the extension .vmf");
        }

        let Ok(mut file) = std::fs::File::open(path)
        else { panic!("mesh: no such file as {path}") };
        
        let mut data = Vec::with_capacity(file.stream_len().unwrap_or(0) as _);
        file.read_to_end(&mut data).unwrap();

        let model = VoxelMesh::decode(&data).unwrap();
        Mesh::new(device, &model.vertices, &model.indices)
    }



    pub fn new(device: &wgpu::Device, vertices: &[voxel_mesher::Vertex], indices: &[u32]) -> Self {
        let vertices_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("mesh-vertex-buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: BufferUsages::VERTEX,
        });


        let indices_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("mesh-index-buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: BufferUsages::INDEX,
        });


        Self {
            vertices: vertices_buffer,
            indices: indices_buffer,
        }
    }
}


pub fn vertex_desc() -> wgpu::VertexBufferLayout<'static> {
    const ATTRS: &[wgpu::VertexAttribute] =
        &wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<voxel_mesher::Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: ATTRS,
    }
}


impl MeshInstance {

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] =
            &wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}
