use std::{collections::HashMap, fs::{self, File}, io::Write};

use glam::{IVec2, Vec2, Vec4};
use rand::seq::IndexedRandom;
use sti::{arena::ArenaStats, define_key, key::Key, vec::KVec};

use crate::shader::ShaderProgram;

use super::{GpuTexture, GpuTextureFormat, UIVertex};

define_key!(TextureListId(u32));


#[derive(Clone, Copy, Debug)]
pub struct TextureId(GpuTextureFormat, TextureListId);


pub struct TextureAtlasBuilder {
    arena: sti::arena::Arena,
    max_dims: IVec2,
    data_format: GpuTextureFormat,

    textures: KVec<TextureListId, (&'static [u8], IVec2)>
}


#[derive(Debug)]
pub struct TextureAtlas {
    uvs: KVec<TextureListId, Vec4>,
    pub(super) gpu_texture: GpuTexture,
}


pub struct TextureAtlasManager {
    pub(super) atlases: HashMap<GpuTextureFormat, (TextureAtlas, ShaderProgram, Vec<UIVertex>)>,
}


impl TextureAtlasManager {
    pub fn new() -> Self {
        Self {
            atlases: HashMap::new(),
        }
    }


    pub fn register(&mut self, atlas: TextureAtlas, shader: ShaderProgram) {
        let prev = self.atlases.insert(atlas.gpu_texture.format, (atlas, shader, vec![]));
        assert!(prev.is_none());
    }


    pub fn get_uv(&self, texture: TextureId) -> Vec4 {
        self.atlases[&texture.0].0.uvs[texture.1]
    }


    pub fn buf(&mut self, texture: TextureId) -> &mut Vec<UIVertex> {
        &mut self.atlases.get_mut(&texture.0).unwrap().2
    }

}



impl TextureAtlasBuilder {
    pub fn new(format: GpuTextureFormat) -> Self {
        Self { max_dims: IVec2::ZERO, arena: sti::arena::Arena::new(), data_format: format, textures: KVec::new() }
    }


    pub fn register(&mut self, dim: IVec2, data: &[u8]) -> TextureId {
        self.max_dims = self.max_dims.max(dim);
        // @TODO: arena fails to allocate here, eventually swap out to arena
        let mut buf = sti::vec::Vec::from_value(data.len(), 0);
        buf.copy_from_slice(data);

        // since it's allocated by the current struct in an arena this is fine
        // so long as we don't give out 'static references to others
        let data = unsafe { core::mem::transmute::<&[u8], &'static [u8]>(buf.leak()) };

        TextureId(self.data_format, self.textures.push((data, dim)))
    }


    pub fn build(self) -> TextureAtlas {
        let maximum_texture_size = unsafe {
            let mut maximum_texture_size = 0;
            gl::GetIntegerv(gl::MAX_TEXTURE_SIZE, &mut maximum_texture_size);
            maximum_texture_size
        };

        let used_area = self.textures.len() as i32 * self.max_dims.y * self.max_dims.x;
        let used_area = used_area as u32 * self.data_format.pixel_size();

        let maximum_texture_size_ilog2 = maximum_texture_size.ilog2();

        let mut best_least_wasted_area = u32::MAX;
        let mut best_dims = 0;
        for power in 0..maximum_texture_size_ilog2 {
            let area = 2u32.pow(power+power);
            if area < used_area {
                continue;
            }

            let wasted_area = area - used_area;

            if wasted_area < best_least_wasted_area {
                best_least_wasted_area = wasted_area;
                best_dims = power;
            } else {
                break;
            }
        }

        let line = 2u32.pow(best_dims);
        let cols = (line as f32 / self.max_dims.x as f32).floor() as u32;
        let rows = (line as f32 / self.max_dims.y as f32).floor() as u32;

        let mut buffer : KVec<u32, u8> = sti::vec::Vec::from_value((line*line) as usize, 0u8);
        let mut uvs = KVec::with_cap(self.textures.len());

        let uv_pixel_size = 1.0 / line as f32;

        let mut i = TextureListId(0);
        'l: for row in 0..rows {
            let row_offset = row * line * self.max_dims.y as u32 * self.data_format.pixel_size();
            for col in 0..cols {
                let col_offset = col * self.max_dims.x as u32 * self.data_format.pixel_size();
                let base = row_offset + col_offset;

                let (texture, dims) = self.textures[i];

                for y in 0..dims.y as u32 {
                    let offset = base + y * line * self.data_format.pixel_size();
                    let slice = &mut buffer[offset..offset + dims.x as u32 * self.data_format.pixel_size()];
                    let stride = (y * dims.x as u32 * self.data_format.pixel_size()) as usize;
                    slice.copy_from_slice(&texture[stride..stride + (dims.x as u32 * self.data_format.pixel_size()) as usize]);
                }

                let uv = Vec4::new(
                    (col*self.max_dims.x as u32) as f32 * uv_pixel_size,
                    (row*self.max_dims.y as u32) as f32 * uv_pixel_size,
                    (col*self.max_dims.x as u32 + dims.x as u32) as f32 * uv_pixel_size,
                    (row*self.max_dims.y as u32 + dims.y as u32) as f32 * uv_pixel_size,
                );

                uvs.push(uv);

                i.0 += 1;
                if i.0 >= self.textures.klen().0 {
                    break 'l;
                }
            }
        }


        let gpu_texture = GpuTexture::new(self.data_format);
        gpu_texture.set_data(IVec2::splat(line as i32), &buffer);

        TextureAtlas {
            uvs,
            gpu_texture,
        }
    }
}
