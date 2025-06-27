use std::{io::{Read, Seek}, mem::offset_of, ptr::null_mut};

use glam::{Vec3, Vec4};
use tracing::warn;
use voxel_mesher::VoxelMesh;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    position: Vec3,
    colour: u32,
}


#[derive(Clone, Debug)]
pub struct Mesh {
    pub indices: u32,
    pub vbo: u32,
    pub vao: u32,
    pub ebo: u32,
}


impl Mesh {
    pub fn from_vmf(path: &str) -> Mesh {
        if !path.ends_with(".vmf") {
            warn!("mesh path should have the extension .vmf");
        }

        let Ok(mut file) = std::fs::File::open(path)
        else { panic!("mesh: no such file as {path}") };
        
        let mut data = Vec::with_capacity(file.stream_len().unwrap_or(0) as _);
        file.read_to_end(&mut data).unwrap();

        let model = VoxelMesh::decode(&data).unwrap();
        Mesh::new(&model.vertices, &model.indices)
    }



    pub fn new(verticies: &[voxel_mesher::Vertex], indicies: &[u32]) -> Self {
        let vao = unsafe { 
            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            vao
        };
        let vbo = unsafe { 
            let mut vbo = 0;
            gl::GenBuffers(1, &mut vbo);
            vbo
        };
        let ebo = unsafe { 
            let mut ebo = 0;
            gl::GenBuffers(1, &mut ebo);
            ebo
        };


        unsafe {
            gl::BindVertexArray(vao);

            // load data into vertex buffer
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(gl::ARRAY_BUFFER, (verticies.len() * size_of::<Vertex>()) as _,
                            verticies.as_ptr().cast(), gl::STATIC_DRAW);

            // load data into element buffer
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
            gl::BufferData(gl::ELEMENT_ARRAY_BUFFER, (indicies.len() * size_of::<u32>()) as _,
                            indicies.as_ptr().cast(), gl::STATIC_DRAW);

            // load uniform information
            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, size_of::<Vertex>() as _, offset_of!(Vertex, position) as _);

            gl::EnableVertexAttribArray(1);
            gl::VertexAttribIPointer(1, 1, gl::UNSIGNED_INT, size_of::<Vertex>() as _, offset_of!(Vertex, colour) as _);

            gl::BindVertexArray(0);

        }

        Self { vao, indices: indicies.len() as _, vbo, ebo }
    }


    pub fn draw(&self) {
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::DrawElements(gl::TRIANGLES, self.indices as _, gl::UNSIGNED_INT, null_mut());
            gl::BindVertexArray(0);
        }
    }


    pub fn destroy(&mut self) {
        self.vao = 0;
        unsafe {
            gl::DeleteVertexArrays(1, &self.vao);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteBuffers(1, &self.ebo);
        }
    }
}


impl Drop for Mesh {
    fn drop(&mut self) {
        assert_eq!(self.vao, 0, "mesh wasn't properly destroyed");
    }
}


impl Vertex {
    pub fn new(pos: Vec3, colour: Vec4) -> Self {

        let r = (colour.x.clamp(0.0, 1.0) * 255.0).round() as u32;
        let g = (colour.y.clamp(0.0, 1.0) * 255.0).round() as u32;
        let b = (colour.z.clamp(0.0, 1.0) * 255.0).round() as u32;
        let a = (1.0 * 255.0f32).round() as u32;

        let colour = (r << 24) | (g << 16) | (b << 8) | a;


        Self { position: pos, colour }
    }


    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] =
            &wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

