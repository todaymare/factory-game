use std::{collections::{hash_map::Entry, HashMap}, io::{BufReader, Read, Seek}, mem::offset_of, panic, ptr::null_mut};

use glam::{IVec3, U8Vec3, USizeVec2, USizeVec3, UVec3, UVec4, Vec3, Vec4};
use obj::{raw::object::Polygon, Obj, ObjResult};
use rand::{random_range, seq::IndexedRandom};
use tracing::{info, warn};
use voxel_mesher::VoxelMesh;

use crate::{directions::Direction, quad::Quad};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    position: Vec3,
    colour: u32,
}


#[derive(Clone, Debug)]
pub struct Mesh {
    pub indices: u32,
    vbo: u32,
    vao: u32,
    ebo: u32,
}


impl obj::FromRawVertex<u32> for Vertex {
    fn process(
        positions: Vec<(f32, f32, f32, f32)>,
        normals: Vec<(f32, f32, f32)>,
        _: Vec<(f32, f32, f32)>,
        polygons: Vec<obj::raw::object::Polygon>,
    ) -> obj::ObjResult<(Vec<Self>, Vec<u32>)> {
        let mut vb = Vec::with_capacity(polygons.len() * 3);
        let mut ib = Vec::with_capacity(polygons.len() * 3);
        {
            let mut cache = HashMap::new();
            let mut map = |pi: usize, ni: usize| -> ObjResult<()> {
                // Look up cache
                let index = match cache.entry((pi, ni)) {
                    // Cache miss -> make new, store it on cache
                    Entry::Vacant(entry) => {
                        let p = positions[pi];
                        let n = normals[ni];
                        let normal = Vec3::from_array(n.into());

                        let light_dir = Vec3::new(0.5, 1.0, 0.5).normalize();
                        let light = n.0 * light_dir.x + n.1 * light_dir.y + n.2 * light_dir.z; // dot product
                        let intensity = 0.6 + light.max(0.0) * 0.4; // ambient + directional
                        let colour = Vec3::splat(intensity);
                        let colour = colour * 0.9 + colour * normal * 0.1;

                        let r = (colour.x.clamp(0.0, 1.0) * 255.0).round() as u32;
                        let g = (colour.y.clamp(0.0, 1.0) * 255.0).round() as u32;
                        let b = (colour.z.clamp(0.0, 1.0) * 255.0).round() as u32;
                        let a = (1.0 * 255.0f32).round() as u32;

                        let colour = (r << 24) | (g << 16) | (b << 8) | a;

                        let vertex = Vertex {
                            position: Vec3::new(p.0, p.1, p.2),
                            //normal,
                            colour,
                        };
                        let index = match u32::try_from(vb.len()) {
                            Ok(val) => val,
                            Err(e) => panic!("{:?}", e),
                        };
                        vb.push(vertex);
                        entry.insert(index);
                        index
                    }
                    // Cache hit -> use it
                    Entry::Occupied(entry) => *entry.get(),
                };
                ib.push(index);
                Ok(())
            };

            for polygon in polygons {
                match polygon {
                    Polygon::P(_) | Polygon::PT(_) => panic!(
                        "Tried to extract normal data which are not contained in the model"
                    ),
                    Polygon::PN(ref vec) if vec.len() == 3 => {
                        for &(pi, ni) in vec {
                            map(pi, ni)?;
                        }
                    }
                    Polygon::PTN(ref vec) if vec.len() == 3 => {
                        for &(pi, _, ni) in vec {
                            map(pi, ni)?;
                        }
                    }
                    _ => panic!(
                        "Model should be triangulated first to be loaded properly"
                    ),
                }
            }
        }
        vb.shrink_to_fit();
        Ok((vb, ib))
    }
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
}


impl Drop for Mesh {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.vao);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteBuffers(1, &self.ebo);
        }
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
}

