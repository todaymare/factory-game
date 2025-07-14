use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use glam::{IVec2, Mat4, Vec4};
use sti::{define_key, vec::KVec};
use wgpu::{BindGroup, Extent3d, RenderPipeline, Sampler, ShaderStages, TextureDimension, TextureFormat, TextureView};

use super::{uniform::Uniform, UIVertex};

define_key!(TextureListId(u32));


#[derive(Clone, Copy, Debug)]
pub struct TextureId(TextureFormat, TextureListId);


pub struct TextureAtlasBuilder {
    arena: sti::arena::Arena,
    max_dims: IVec2,
    data_format: TextureFormat,

    textures: KVec<TextureListId, (&'static [u8], IVec2)>
}


#[derive(Debug)]
pub struct TextureAtlas {
    uvs: KVec<TextureListId, Vec4>,
    pub view: TextureView,
    pub sampler: Sampler,
    format: TextureFormat,
}


pub struct UiTextureAtlasManager {
    pub(super) atlases: HashMap<TextureFormat, (TextureAtlas, RenderPipeline, BindGroup, Vec<UIVertex>)>,
    pub ui_shader_uniform: Uniform<UiShaderUniform>
}


#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct UiShaderUniform {
    pub projection: Mat4,
    pub view: Vec4,
}


impl UiTextureAtlasManager {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            atlases: HashMap::new(),
            ui_shader_uniform: Uniform::new("ui-texture-atlas-uniform", device, 0, ShaderStages::VERTEX_FRAGMENT),
        }
    }


    pub fn register(&mut self, atlas: TextureAtlas, shader: RenderPipeline, bg: BindGroup) {
        let prev = self.atlases.insert(atlas.format, (atlas, shader, bg, vec![]));
        assert!(prev.is_none());
    }


    pub fn get_uv(&self, texture: TextureId) -> Vec4 {
        self.atlases[&texture.0].0.uvs[texture.1]
    }


    pub fn buf(&mut self, texture: TextureId) -> &mut Vec<UIVertex> {
        &mut self.atlases.get_mut(&texture.0).unwrap().3
    }

}



impl TextureAtlasBuilder {
    pub fn new(format: TextureFormat) -> Self {
        Self { max_dims: IVec2::ZERO, arena: sti::arena::Arena::new(), data_format: format, textures: KVec::new() }
    }


    pub fn register(&mut self, dim: IVec2, data: &[u8]) -> TextureId {
        let pixel_size = self.data_format.block_copy_size(Some(wgpu::TextureAspect::All)).unwrap();
        assert_eq!(dim.x * dim.y * pixel_size as i32, data.len() as i32,
                   "format: {:?}, pixel_size: {pixel_size}, dims: {dim}", self.data_format);
        self.max_dims = self.max_dims.max(dim);
        let mut buf = sti::vec::Vec::from_value_in(&self.arena, data.len(), 0);
        buf.copy_from_slice(data);

        // since it's allocated by the current struct in an arena this is fine
        // so long as we don't give out 'static references to others
        let data = unsafe { core::mem::transmute::<&[u8], &'static [u8]>(buf.leak()) };

        TextureId(self.data_format, self.textures.push((data, dim)))
    }


    pub fn build(self, device: &wgpu::Device, queue: &wgpu::Queue) -> TextureAtlas {
        let maximum_texture_size = device.limits().max_texture_dimension_2d;

        let pixel_size = self.data_format.target_pixel_byte_cost().unwrap();
        let pixel_size = self.data_format.block_copy_size(Some(wgpu::TextureAspect::All)).unwrap();
        let used_area = self.textures.len() as i32 * self.max_dims.y * self.max_dims.x;
        let used_area = used_area as u32 * pixel_size;

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

        let mut buffer : KVec<u32, u8> = sti::vec::Vec::from_value((line*line*pixel_size) as usize, 0u8);
        let mut uvs = KVec::with_cap(self.textures.len());

        let uv_pixel_size = 1.0 / line as f32;

        let mut i = TextureListId(0);
        'l: for row in 0..rows {
            let row_offset = row * line * self.max_dims.y as u32 * pixel_size;
            for col in 0..cols {
                let col_offset = col * self.max_dims.x as u32 * pixel_size;
                let base = row_offset + col_offset;

                let (texture, dims) = self.textures[i];

                for y in 0..dims.y as u32 {
                    let offset = base + y * line * pixel_size;
                    let slice = &mut buffer[offset..offset + dims.x as u32 * pixel_size];
                    let stride = (y * dims.x as u32 * pixel_size) as usize;
                    slice.copy_from_slice(&texture[stride..stride + (dims.x as u32 * pixel_size) as usize]);
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


        let texture_size = Extent3d {
            width: line,
            height: line,
            depth_or_array_layers: 1,
        };

        let diffuse_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture-atlas-texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.data_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let diffuse_texture_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture-atlas-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        dbg!(pixel_size, &diffuse_texture);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &diffuse_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &buffer,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pixel_size*line),
                rows_per_image: Some(line),
            },

            texture_size
        );


        TextureAtlas {
            uvs,
            view: diffuse_texture_view,
            sampler: diffuse_sampler,
            format: self.data_format,
        }
    }
}
