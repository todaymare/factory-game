use std::{mem::offset_of, ptr::null_mut};

use glam::{UVec3, UVec4, Vec3, Vec4};
use obj::{raw::object::Polygon, Obj, ObjResult};

use crate::{directions::Direction, quad::Quad};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    data1: u32,
    data2: u32,
    //colour: Vec4,
}


#[derive(Debug)]
pub struct VoxelMesh {
    pub indices: u32,
    vbo: u32,
    vao: u32,
    ebo: u32,
}


impl VoxelMesh {
    pub fn new(verticies: &[Vertex], indicies: &[u32]) -> Self {
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
            gl::VertexAttribIPointer(0, 1, gl::UNSIGNED_INT, size_of::<Vertex>() as _, offset_of!(Vertex, data1) as _);

            gl::EnableVertexAttribArray(1);
            gl::VertexAttribIPointer(1, 1, gl::UNSIGNED_INT, size_of::<Vertex>() as _, offset_of!(Vertex, data2) as _);

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
}


impl Drop for VoxelMesh {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.vao);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteBuffers(1, &self.ebo);
        }
    }
}


impl Vertex {
    pub fn new(pos: Vec3, normal: Vec3, colour: Vec4) -> Self {
        let UVec3 { x, y, z } = pos.as_uvec3();
        let UVec4 { x: r, y: g, z: b, w: a } = (colour * 255.0).as_uvec4();

        assert!(x <= 32 && y <= 32 && z <= 32, "{x} {y} {z}");
        let pos = ((z as u32) << 12) | ((y as u32) << 6) | (x as u32);
        let colour = ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32);

        let data1 = pos as u32;
        let data2 = colour as u32;

        Self { data1, data2 }
    }
}


pub fn draw_quad(verticies: &mut Vec<Vertex>, indicies: &mut Vec<u32>, quad: Quad) {
    let normal = match quad.direction {
        Direction::Left => Vec3::new(1.0, 0.0, 0.0),
        Direction::Right => Vec3::new(-1.0, 0.0, 0.0),
        Direction::Down => Vec3::new(0.0, -1.0, 0.0),
        Direction::Up => Vec3::new(0.0, 1.0, 0.0),
        Direction::Back => Vec3::new(0.0, 0.0, -1.0),
        Direction::Forward => Vec3::new(0.0, 0.0, 1.0),
    };

    let k = verticies.len() as u32;
    let mut i = 0;
    for corner in quad.corners {
        let mut colour = quad.color;
        colour = colour * 0.9 + colour * (i as f32 * 0.1);
        colour.w = quad.color.w;
        verticies.push(Vertex::new(Vec3::new(corner[0] as f32, corner[1] as f32, corner[2] as f32), normal, colour));
        i += 1;
    }


    indicies.extend_from_slice(&[k, k+1, k+2, k+2, k+3, k]);
}
