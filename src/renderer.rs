pub mod textures;
pub mod uniform;
pub mod ssbo;
pub mod gpu_allocator;

use std::{cell::Cell, collections::HashMap, mem::offset_of, ops::{Deref, DerefMut}, ptr::null_mut, time::{SystemTime, UNIX_EPOCH}};

use bytemuck::{Pod, Zeroable};
use glam::{IVec2, IVec3, Mat4, UVec3, Vec2, Vec2Swizzles, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles};
use gpu_allocator::GPUAllocator;
use image::{EncodableLayout, GenericImage, GenericImageView, RgbaImage};
use ssbo::{ResizableBuffer, SSBO};
use sti::{key::Key, static_assert_eq, vec::KVec};
use textures::{TextureAtlasBuilder, TextureId, UiShaderUniform, UiTextureAtlasManager};
use tracing::warn;
use uniform::Uniform;
use wgpu::{util::{BufferInitDescriptor, DeviceExt, StagingBelt}, wgt::DrawIndirectArgs, BufferUsages, TextureUsages, *};
use winit::window::Window;

use crate::{constants::{CHUNK_SIZE, FONT_SIZE, MSAA_SAMPLE_COUNT, QUAD_VERTICES, UI_DELTA_Z, UI_Z_MAX, UI_Z_MIN, VOXEL_TEXTURE_ATLAS_TILE_CAP, VOXEL_TEXTURE_ATLAS_TILE_SIZE}, directions::CardinalDirection, free_list::FreeKVec, frustum::Frustum, items::{Assets, ItemKind, MeshIndex}, mesh::MeshInstance, voxel_world::{chunker::ChunkPos, mesh::{ChunkMeshFramedata, ChunkQuadInstance, VoxelMeshIndex}, split_world_pos, VoxelWorld}, Camera};


// the renderer is done,
// never to be touched until the heat death of the universe
// ..or shadows need casting
// whichever comes first

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub window: &'static Window,

    pub framebuffer: wgpu::TextureView,
    pub ui_depth_texture: DepthBuffer,

    pub voxel_pipeline: VoxelPipeline,
    pub mesh_pipeline: MeshPipeline,

    pub staging_buffer: StagingBelt,


    pub ui_scale: f32,
    pub rects: Vec<DrawRect>,

    pub draw_count: Cell<u32>,
    pub triangle_count: Cell<u32>,

    pub ui_atlases: UiTextureAtlasManager,

    pub line_size: f32,
    pub characters: HashMap<char, Character>,
    pub white_texture: TextureId,
    pub ui_vertex_buff: ResizableBuffer<UIVertex>,

    pub mesh_draws: KVec<MeshIndex, Vec<MeshInstance>>,
    pub assets: Assets,
}


pub struct DrawRect {
    modulate: Vec4,
    pos: Vec2,
    dims: Vec2,
    tex: TextureId,
    z: Option<f32>,
}


#[derive(Debug)]
pub struct Character {
    pub texture: TextureId,
    pub size: IVec2,
    pub bearing: IVec2,
    pub advance: u32,
}


pub struct RenderSettings<'a> {
    pub camera: &'a Camera,
    pub skybox: Vec4,
    pub render_distance: u32,
    pub frustum: Option<Frustum>,
    pub lines: bool,
}


#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct VoxelShaderUniform {
    pub view: Mat4,
    pub projection: Mat4,
    pub modulate: Vec4,
    pub camera_block: IVec3,
    pub pad_00: f32,
    pub camera_offset: Vec3,
    pub pad_01: f32,
    pub fog_color: Vec3,
    pub pad_02: f32,
    pub fog_density: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub pad_03: f32,
}

static_assert_eq!(size_of::<VoxelShaderUniform>(), 208);


#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct MeshShaderUniform {
    pub view: Mat4,
    pub projection: Mat4,
}


pub struct VoxelPipeline {
    pub pipeline: RenderPipeline,
    pub line_pipeline: RenderPipeline,
    pub frame_uniform: Uniform<VoxelShaderUniform>,
    pub model_uniform: SSBO<ChunkMeshFramedata>,
    pub depth_buffer: DepthBuffer,

    pub chunk_offsets: FreeKVec<VoxelMeshIndex, ChunkMeshFramedata>,
    pub instances: GPUAllocator<ChunkQuadInstance>,
    pub indirect_buf: ResizableBuffer<DrawIndirectArgs>,
    pub vertex_buf: Buffer,

    pub texture: BindGroup,
}


pub struct MeshPipeline {
    pub pipeline: RenderPipeline,
    pub line_pipeline: RenderPipeline,
    pub frame_uniform: Uniform<MeshShaderUniform>,

    pub instance_buffer: ResizableBuffer<MeshInstance>,
}


impl Renderer {
    pub async fn new(window: Window) -> Self {
        let window = Box::leak(Box::new(window));

        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(&*window).unwrap();

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }
        ).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::POLYGON_MODE_LINE
                                    | wgpu::Features::TEXTURE_BINDING_ARRAY
                                    | wgpu::Features::STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING
                                    | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
                                    | wgpu::Features::MULTI_DRAW_INDIRECT
                                    | wgpu::Features::INDIRECT_FIRST_INSTANCE
                                    | wgpu::Features::TIMESTAMP_QUERY,
                required_limits: {
                    let mut limits = wgpu::Limits::downlevel_defaults();
                    limits.max_buffer_size = adapter.limits().max_buffer_size;
                    limits.max_storage_buffer_binding_size = 512 << 20;
                    limits.max_texture_dimension_2d = 8192;
                    limits
                },
                label: Some("main device"),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off
            },
        ).await.unwrap();

        let surface_capabilities = surface.get_capabilities(&adapter);

        let surface_format = surface_capabilities.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_capabilities.formats[0]);


        let config = wgpu::SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: surface_capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);


        let mesh_pipeline = {
            let mesh_shader_uniform = Uniform::<MeshShaderUniform>::new("mesh-shader-frame-uniform", &device, 0, ShaderStages::VERTEX_FRAGMENT);

            let shader = device.create_shader_module(
                wgpu::ShaderModuleDescriptor {
                    label: Some("mesh-shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/mesh.wgsl").into()),
                }
            );

            let rpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("mesh-render-pipeline-layout"),
                bind_group_layouts: &[mesh_shader_uniform.bind_group_layout()],
                push_constant_ranges: &[],
            });


            let targets = &[Some(wgpu::ColorTargetState { // 4.
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })];

            let mut desc = wgpu::RenderPipelineDescriptor {
                label: Some("mesh-render-pipeline"),
                layout: Some(&rpl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"), // 1.
                    buffers: &[
                        crate::mesh::vertex_desc(),
                        MeshInstance::desc(),
                    ], 
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState { // 3.
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw, // 2.
                    cull_mode: Some(Face::Back),
                    //cull_mode: None,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    // Requires Features::DEPTH_CLIP_CONTROL
                    unclipped_depth: false,
                    // Requires Features::CONSERVATIVE_RASTERIZATION
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: MSAA_SAMPLE_COUNT, // 2.
                    mask: !0, // 3.
                    alpha_to_coverage_enabled: false, // 4.
                },
                multiview: None, // 5.
                cache: None, // 6.
            };


            let render_pipeline = device.create_render_pipeline(&desc);
            desc.primitive.polygon_mode = wgpu::PolygonMode::Line;
            let line_render_pipeline = device.create_render_pipeline(&desc);

            let instance_buffer = ResizableBuffer::new("mesh-instance-buffer", &device, BufferUsages::COPY_SRC | BufferUsages::COPY_DST | BufferUsages::VERTEX, 128);

            MeshPipeline {
                pipeline: render_pipeline,
                line_pipeline: line_render_pipeline,
                frame_uniform: mesh_shader_uniform,
                instance_buffer,
            }
        };




        let voxel_pipeline = {
            let voxel_shader_uniform = Uniform::<VoxelShaderUniform>::new("voxel-shader-frame-uniform", &device, 0, ShaderStages::VERTEX_FRAGMENT);
            let ssbo = SSBO::new("voxel-shader-chunk-offsets-ssbo", &device, BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::STORAGE, 16 * 1024 * 400);

            let shader = device.create_shader_module(
                wgpu::ShaderModuleDescriptor {
                    label: Some("voxel-shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/voxel.wgsl").into()),
                }
            );



            let diffuse_bytes = include_bytes!("../textures.png");
            let diffuse_image = image::load_from_memory(diffuse_bytes).unwrap();
            let diffuse_image = diffuse_image.flipv();

            let dims = diffuse_image.dimensions();
            assert_eq!(dims.0, VOXEL_TEXTURE_ATLAS_TILE_SIZE * VOXEL_TEXTURE_ATLAS_TILE_CAP);
            assert_eq!(dims.1, VOXEL_TEXTURE_ATLAS_TILE_SIZE);

            let texture_size = wgpu::Extent3d {
                width: dims.0,
                height: dims.1,
                depth_or_array_layers: 1,
            };

            let mipmap_count = 1 + VOXEL_TEXTURE_ATLAS_TILE_SIZE.ilog2();

            let diffuse_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("diffuse-texture"),
                size: texture_size,
                mip_level_count: mipmap_count,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });


            let mut mipmap_visual_image = RgbaImage::new(
                dims.0,
                (0..mipmap_count).map(|i| dims.1 / (2u32.pow(i))).sum(),
            );

            let mut mipmap_visual_y_offset = 0;

            for i in 0..mipmap_count {
                let dims = if i == 0 { dims }
                else { (dims.0 / (2u32.pow(i)), dims.1 / (2u32.pow(i))) };

                let mut mipmap_image = RgbaImage::new(dims.0, dims.1);

                for offset in 0..VOXEL_TEXTURE_ATLAS_TILE_CAP {
                    let base = offset * VOXEL_TEXTURE_ATLAS_TILE_SIZE;
                    let diffuse_image = diffuse_image.crop_imm(base, 0, 32, 32);
                    let diffuse_image = diffuse_image.resize_exact(dims.1, dims.1, image::imageops::FilterType::Lanczos3);
                    mipmap_image.copy_from(&diffuse_image, offset*dims.1, 0).unwrap();
                }


                mipmap_visual_image.copy_from(&mipmap_image, 0, mipmap_visual_y_offset).unwrap();
                mipmap_visual_y_offset += dims.1;

                let texture_size = wgpu::Extent3d {
                    width: dims.0,
                    height: dims.1,
                    depth_or_array_layers: 1,
                };

                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &diffuse_texture,
                        mip_level: i,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &mipmap_image.as_bytes(),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4*dims.0),
                        rows_per_image: Some(dims.1),
                    },

                    texture_size
                );
            }

            mipmap_visual_image.save("mipmaps.png").unwrap();

            let diffuse_texture_view = diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("diffuse-sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });


            let texture_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("texutre-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

            let diffuse_bind_group = device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    layout: &texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                        }
                    ],
                    label: Some("diffuse-bind-group"),
                }
            );



            let rpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("voxel-render-pipeline-layout"),
                bind_group_layouts: &[voxel_shader_uniform.bind_group_layout(), ssbo.layout(), &texture_bind_group_layout],
                push_constant_ranges: &[],
            });


            let depth_texture = DepthBuffer::new(&device, config.width, config.height, MSAA_SAMPLE_COUNT); 

            let targets = &[Some(wgpu::ColorTargetState { // 4.
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })];
            let mut desc = wgpu::RenderPipelineDescriptor {
                label: Some("voxel-render-pipeline"),
                layout: Some(&rpl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"), // 1.
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: 16,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Sint32x3,
                                    offset: 0,
                                    shader_location: 0,
                                },
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Uint32,
                                    offset: 12,
                                    shader_location: 1,
                                },
                            ],
                        },
                        ChunkQuadInstance::desc(),
                    ], 
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState { // 3.
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Cw, // 2.
                    cull_mode: Some(wgpu::Face::Back),
                    //cull_mode: None,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    // Requires Features::DEPTH_CLIP_CONTROL
                    unclipped_depth: false,
                    // Requires Features::CONSERVATIVE_RASTERIZATION
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: MSAA_SAMPLE_COUNT, // 2.
                    mask: !0, // 3.
                    alpha_to_coverage_enabled: false, // 4.
                },
                multiview: None, // 5.
                cache: None, // 6.
            };


            let render_pipeline = device.create_render_pipeline(&desc);
            desc.primitive.polygon_mode = wgpu::PolygonMode::Line;
            let line_render_pipeline = device.create_render_pipeline(&desc);


            let vertex = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("quad-vertices"),
                    usage: BufferUsages::VERTEX,
                    contents: bytemuck::cast_slice(QUAD_VERTICES),
                });

            let indirect = ResizableBuffer::new(
                "voxel-shader-indirect-buffer",
                &device,
                BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                1024
            );

            VoxelPipeline {
                pipeline: render_pipeline,
                line_pipeline: line_render_pipeline,
                frame_uniform: voxel_shader_uniform,
                model_uniform: ssbo,
                depth_buffer: depth_texture,
                vertex_buf: vertex,
                instances: GPUAllocator::new(&device, 1),
                indirect_buf: indirect,
                chunk_offsets: FreeKVec::new(),
                texture: diffuse_bind_group,
            }
        };



        let mut ui_atlases = UiTextureAtlasManager::new(&device);
        let line_size;
        let characters;
        let white;
        let ui_depth_texture = DepthBuffer::new(&device, config.width, config.height, 1); 

        {


            let mut chars = HashMap::new();
            let mut atlas = TextureAtlasBuilder::new(TextureFormat::R8Unorm);

            let mut biggest_y_size : f32 = 0.0;


            let mut ft = null_mut();
            if unsafe { freetype::freetype::FT_Init_FreeType(&mut ft) } != 0 {
                panic!("failed to init freetype library");
            }


            let mut face = null_mut();
            if unsafe { freetype::freetype::FT_New_Face(ft, c"font.ttf".as_ptr(), 0, &mut face) } != 0 {
                panic!("failed to load font");
            }

            unsafe { freetype::freetype::FT_Set_Pixel_Sizes(face, FONT_SIZE, FONT_SIZE) };
            for c in 0..128 {
                if unsafe { freetype::freetype::FT_Load_Char(face, c as u64, freetype::freetype::FT_LOAD_RENDER as _) } != 0 {
                    panic!("failed to load glyph '{}'", char::from_u32(c).unwrap());
                }


                unsafe {
                    let dims = IVec2::new(
                        (*(*face).glyph).bitmap.width as _,
                        (*(*face).glyph).bitmap.rows as _,
                    );

                    let slice = if dims != IVec2::ZERO { core::slice::from_raw_parts((*(*face).glyph).bitmap.buffer, (dims.x * dims.y) as usize) }
                                else { &[] };
                    let texture = atlas.register(dims, slice);

                    let character = Character {
                        texture,
                        size: IVec2::new(
                            (*(*face).glyph).bitmap.width as _,
                            (*(*face).glyph).bitmap.rows as _,
                        ),
                        bearing: IVec2::new(
                            (*(*face).glyph).bitmap_left as _,
                            (*(*face).glyph).bitmap_top as _,
                        ),
                        advance: (*(*face).glyph).advance.x as _,
                    };

                    let h = character.size.y as f32;
                    if h > biggest_y_size {
                        dbg!(char::from_u32(c).unwrap());
                    }

                    biggest_y_size = biggest_y_size.max(h);

                    chars.insert(char::from_u32(c).unwrap(), character);
                }
            }

            unsafe {
                freetype::freetype::FT_Done_Face(face);
                freetype::freetype::FT_Done_FreeType(ft);
            }

            white = atlas.register(IVec2::new(32, 32), &[255; 32*32]);
            line_size = biggest_y_size;
            characters = chars;
            let atlas = atlas.build(&device, &queue);

            let shader = device.create_shader_module(
                wgpu::ShaderModuleDescriptor {
                    label: Some("font-shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/text.wgsl").into()),
                }
            );


            let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("font-texture-atlas-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });


            let bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("font-texture-bind-group"),
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&atlas.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                    }
                ]
            });


            let rpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("font-render-pipeline-layout"),
                bind_group_layouts: &[ui_atlases.ui_shader_uniform.bind_group_layout(), &bgl],
                push_constant_ranges: &[],
            });

            let targets = [Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })];

            let desc = wgpu::RenderPipelineDescriptor{
                label: Some("font-render-pipeline-descriptor"),
                layout: Some(&rpl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[
                        UIVertex::desc(),
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &targets,
                }),

                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Cw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: PolygonMode::Fill,
                    conservative: false,
                },

                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),

                multisample: wgpu::MultisampleState {
                    count: 1, // 2.
                    mask: !0, // 3.
                    alpha_to_coverage_enabled: false, // 4.
                },
                multiview: None,
                cache: None,
            };

            let render_pipeline = device.create_render_pipeline(&desc);
            ui_atlases.register(atlas, render_pipeline, bg);
        }

        let framebuffer = create_multisampled_framebuffer(&device, &config);


        let mut assets_ta = TextureAtlasBuilder::new(TextureFormat::Rgba8UnormSrgb);
        let assets = Assets::new(&device, &mut assets_ta);
        let assets_ta = assets_ta.build(&device, &queue);


        {
            let shader = device.create_shader_module(
                wgpu::ShaderModuleDescriptor {
                    label: Some("ui-shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/ui.wgsl").into()),
                }
            );


            let bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("ui-texture-atlas-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });


            let bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("ui-texture-bind-group"),
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&assets_ta.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&assets_ta.sampler),
                    }
                ]
            });


            let rpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ui-render-pipeline-layout"),
                bind_group_layouts: &[ui_atlases.ui_shader_uniform.bind_group_layout(), &bgl],
                push_constant_ranges: &[],
            });

            let targets = [Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })];

            let desc = wgpu::RenderPipelineDescriptor{
                label: Some("ui-render-pipeline-descriptor"),
                layout: Some(&rpl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[
                        UIVertex::desc(),
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &targets,
                }),

                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Cw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: PolygonMode::Fill,
                    conservative: false,
                },

                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),

                multisample: wgpu::MultisampleState {
                    count: 1, // 2.
                    mask: !0, // 3.
                    alpha_to_coverage_enabled: false, // 4.
                },
                multiview: None,
                cache: None,
            };

            let render_pipeline = device.create_render_pipeline(&desc);
            ui_atlases.register(assets_ta, render_pipeline, bg);
        }



        let this = Self {
            window,
            ui_vertex_buff: ResizableBuffer::new("ui-vertex-buffer", &device, BufferUsages::VERTEX | BufferUsages::COPY_SRC | BufferUsages::COPY_DST, 128),
            ui_scale: 1.0,
            rects: vec![],
            draw_count: Cell::new(0),
            triangle_count: Cell::new(0),
            surface,
            device,
            queue,
            config,
            mesh_pipeline,
            voxel_pipeline,
            staging_buffer: StagingBelt::new(128 << 20),
            framebuffer,
            ui_atlases,
            line_size,
            characters,
            white_texture: white,

            mesh_draws: KVec::new(),
            assets,
            ui_depth_texture,
        };

        this
    }


    pub fn end(&mut self, mut encoder: wgpu::CommandEncoder, voxel_world: &mut VoxelWorld, output_texture: &TextureView, settings: RenderSettings) {
        let framebuffer = &self.framebuffer;


        let camera = settings.camera.position;
        let projection = settings.camera.perspective_matrix();
        let view = settings.camera.view_matrix();


        self.staging_buffer.recall();


        let triangle_count = self.triangle_count.get_mut();
        // prepare voxel buffers
        let indirect_len;
        {
            let voxel_pipeline = &mut self.voxel_pipeline;

            let (player_chunk, _) = split_world_pos(camera.as_ivec3());

            let mut indirect : Vec<DrawIndirectArgs> = vec![];

            let frustum = match &settings.frustum{
                Some(f) => f.clone(),
                None => Frustum::compute(projection, view),
            };


            let mut buf = vec![];
            for (pos, region) in voxel_world.chunker.regions() {
                region.octree().render(
                    ChunkPos(UVec3::ZERO),
                    pos,
                    player_chunk,
                    camera,
                    &frustum,
                    &mut indirect,
                    &mut buf,
                    settings.render_distance as i32,
                    triangle_count,
                );
            }


            for b in buf { voxel_world.chunker.get_mesh_or_queue(b); }

            if !indirect.is_empty() {
                voxel_pipeline.indirect_buf.resize(&self.device, &mut encoder, indirect.len());
                voxel_pipeline.indirect_buf.write(&mut self.staging_buffer, &mut encoder, &self.device, 0, &indirect);
            }

            indirect_len = indirect.len();
        }


        // prepare mesh buffers
        'meshes: {
            let mut buf = vec![];

            // upload instances
            for (_, instances) in &mut self.mesh_draws {
                for instance in instances {
                    if instance.modulate.w == 1.0 {
                        buf.push(*instance);
                    }
                }
            }


            for (_, instances) in &mut self.mesh_draws {
                for instance in instances {
                    if instance.modulate.w != 1.0 {
                        buf.push(*instance);
                    }
                }
            }

            if buf.is_empty() { break 'meshes }

            self.mesh_pipeline.instance_buffer.resize(&self.device, &mut encoder, buf.len());
            self.mesh_pipeline.instance_buffer.write(&mut self.staging_buffer, &mut encoder, &self.device, 0, &buf);
        }


        let c = settings.skybox.as_dvec4();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("world-render-pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &framebuffer,
                    resolve_target: Some(&output_texture),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: c.x, g: c.y, b: c.z, a: c.w }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                }),
            ],

            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.voxel_pipeline.depth_buffer.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),

                stencil_ops: None,
            }),

            ..Default::default()
        });




        // render voxel world
        {
            let rd = settings.render_distance;
            let fog_distance = (rd - 1) as f32;

            let uniform = VoxelShaderUniform {
                view,
                projection,
                modulate: Vec4::ONE,

                camera_block: camera.floor().as_ivec3(),
                camera_offset: (camera - camera.floor()).as_vec3(),

                fog_color: settings.skybox.xyz(),
                fog_density: 1.0,
                fog_start: fog_distance * CHUNK_SIZE as f32 * 0.9,
                fog_end: fog_distance * CHUNK_SIZE as f32,
                pad_00: 0.0,
                pad_01: 0.0,
                pad_02: 0.0,
                pad_03: 0.0,
            };

            let voxel_pipeline = &mut self.voxel_pipeline;

            pass.set_pipeline(if settings.lines { &voxel_pipeline.line_pipeline } else { &voxel_pipeline.pipeline });

            voxel_pipeline.frame_uniform.update(&self.queue, &uniform);
            voxel_pipeline.frame_uniform.use_uniform(&mut pass);
            pass.set_bind_group(1, voxel_pipeline.model_uniform.bind_group(), &[]);
            pass.set_bind_group(2, &voxel_pipeline.texture, &[]);

            pass.set_vertex_buffer(0, voxel_pipeline.vertex_buf.slice(..));
            pass.set_vertex_buffer(1, voxel_pipeline.instances.ssbo.buffer.slice(..));
            pass.multi_draw_indirect(&voxel_pipeline.indirect_buf.buffer, 0, indirect_len as _);
        }


        // draw meshes
        {
            pass.set_pipeline(if settings.lines { &self.mesh_pipeline.line_pipeline } else { &self.mesh_pipeline.pipeline });

            self.mesh_pipeline.frame_uniform.update(&self.queue, &MeshShaderUniform {
                view,
                projection,
            });

            self.mesh_pipeline.frame_uniform.use_uniform(&mut pass);
            pass.set_vertex_buffer(1, self.mesh_pipeline.instance_buffer.buffer.slice(..));

            let mut counter = 0;
            for (index, instances) in &mut self.mesh_draws {
                if instances.is_empty() { continue }

                let mesh = &self.assets.meshes[index];

                *triangle_count += mesh.index_count * instances.len() as u32;

                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), IndexFormat::Uint32);

                let len = instances.iter().filter(|x| x.modulate.w == 1.0).count() as u32;
                pass.draw_indexed(0..mesh.index_count, 0, counter..counter+len);
                counter += len;

            }

            for (index, instances) in &mut self.mesh_draws {
                if instances.is_empty() { continue }

                let mesh = &self.assets.meshes[index];
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), IndexFormat::Uint32);

                let len = instances.iter().filter(|x| x.modulate.w != 1.0).count() as u32;
                pass.draw_indexed(0..mesh.index_count, 0, counter..counter+len);
                counter += len;
                instances.clear();

            }


        }




        drop(pass);

        // draw UI
        let mut z = UI_Z_MIN;

        for rect in self.rects.iter() {
            let tex = rect.tex;
            let pos = rect.pos;
            let dims = rect.dims;
            let modulate = rect.modulate;

            let uvs = self.ui_atlases.get_uv(tex);
            let buf = self.ui_atlases.buf(tex);

            let x0 = uvs.x;
            let y0 = uvs.y;
            let x1 = uvs.z;
            let y1 = uvs.w;

            let z = if let Some(rect_z) = rect.z { rect_z } else { z += UI_DELTA_Z; z };
            assert!(z > UI_Z_MIN && z <= UI_Z_MAX);

            buf.push(UIVertex::new(pos+dims, Vec2::new(x1, y1), modulate, z));
            buf.push(UIVertex::new(pos+Vec2::new(dims.x, 0.0), Vec2::new(x1, y0), modulate, z));
            buf.push(UIVertex::new(pos+Vec2::new(0.0, dims.y), Vec2::new(x0, y1), modulate, z));

            buf.push(UIVertex::new(pos+Vec2::new(dims.x, 0.0), Vec2::new(x1, y0), modulate, z));
            buf.push(UIVertex::new(pos, Vec2::new(x0, y0), modulate, z));
            buf.push(UIVertex::new(pos+Vec2::new(0.0, dims.y), Vec2::new(x0, y1), modulate, z));

            *triangle_count += 6;
        }

        let mut vertex_buf = vec![];
        for (_, (_, _, _, buf)) in self.ui_atlases.atlases.iter() {
            vertex_buf.extend(buf);
        }

        let size = self.window_size();
        let projection = glam::Mat4::orthographic_rh(0.0, size.x as f32, size.y as f32, 0.0, -UI_Z_MAX, UI_Z_MIN);

        let curr = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let view = Vec4::splat(curr.as_secs_f64().sin() as f32 * 0.5 + 0.5);

        self.ui_atlases.ui_shader_uniform.update(&self.queue, &UiShaderUniform { projection, view });

        self.ui_vertex_buff.resize(&self.device, &mut encoder, vertex_buf.len());
        self.ui_vertex_buff.write(&mut self.staging_buffer, &mut encoder, &self.device, 0, &vertex_buf);


        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("atlas-render-pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: &output_texture,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })
            ],

            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.ui_depth_texture.view,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(1.0),
                    store: StoreOp::Discard,
                }),
                stencil_ops: None
            }),

            ..Default::default()
        });



        self.ui_atlases.ui_shader_uniform.use_uniform(&mut pass);
        pass.set_vertex_buffer(0, self.ui_vertex_buff.buffer.slice(..));

        let mut counter = 0;
        for (_, (_, render_pipeline, bg, buff)) in self.ui_atlases.atlases.iter_mut() {
            pass.set_pipeline(render_pipeline);
            pass.set_bind_group(1, &*bg, &[]);
            pass.draw(counter..counter + buff.len() as u32, 0..1);

            counter += buff.len() as u32;
            buff.clear();
        }

        drop(pass);

        self.rects.clear();
        self.staging_buffer.finish();

        self.queue.submit(std::iter::once(encoder.finish()));
    }


    pub fn to_point(&self, pos: Vec2) -> Vec2 {
        pos / self.ui_scale
    }


    pub fn draw_text(&mut self, text: &str, pos: Vec2, scale: f32, default_colour: Vec4) {
        self.draw_text_ex(text, pos, scale, default_colour, false);
    }

    pub fn draw_text_ex(&mut self, text: &str, pos: Vec2, scale: f32, default_colour: Vec4, discard_colour_codes: bool) {
        let mut x;
        let mut y = pos.y;
        let mut active_colour = default_colour;

        for l in text.lines() {
            x = pos.x;
            y += self.line_size * scale;


            let mut line_size = 0.0f32;
            let mut iter = l.chars();
            while let Some(c) = iter.next() {
                if c == '§' {
                    let colour_code = iter.next().unwrap();

                    if discard_colour_codes { continue };
                    active_colour = match colour_code {
                        '0' => Vec4::ZERO,
                        '1' => Vec4::new(0.0, 0.0, 0.4, 1.0),
                        '2' => Vec4::new(0.0, 0.4, 0.0, 1.0),
                        '3' => Vec4::new(0.0, 0.4, 0.4, 1.0),
                        '4' => Vec4::new(0.4, 0.0, 0.0, 1.0),
                        '5' => Vec4::new(0.4, 0.0, 0.4, 1.0),
                        '6' => Vec4::new(1.0, 0.4, 0.0, 1.0),
                        '7' => Vec4::new(0.4, 0.4, 0.4, 1.0),
                        '8' => Vec4::new(0.1, 0.1, 0.1, 1.0),
                        '9' => Vec4::new(0.1, 0.1, 1.0, 1.0),
                        'a' => Vec4::new(0.1, 1.0, 0.1, 1.0),
                        'b' => Vec4::new(0.1, 1.0, 1.0, 1.0),
                        'c' => Vec4::new(1.0, 0.1, 0.1, 1.0),
                        'd' => Vec4::new(1.0, 0.1, 1.0, 1.0),
                        'e' => Vec4::new(1.0, 1.0, 0.5, 1.0),
                        'f' => Vec4::ONE,
                        'r' => default_colour,

                        _ => {
                            warn!("invalid colour code '§{}', resetting to default colour", colour_code);
                            default_colour
                        },
                    };
                    continue
                }

                let Some(ch) = self.characters.get(&c)
                else { warn!("[renderer] draw-text: character not registered '{c}'"); continue };

                let xpos = x + ch.bearing.x as f32 * scale;
                let ypos = y - (ch.size.y + ch.bearing.y) as f32 * scale * 0.5;
                x += (ch.advance >> 6) as f32 * scale;

                let w = ch.size.x as f32 * scale;
                let h = ch.size.y as f32 * scale;
                line_size = line_size.max(h);

                let dims = Vec2::new(w, h);
                self.draw_tex_rect(Vec2::new(xpos, ypos), dims, ch.texture, active_colour.with_w(default_colour.w));
            }


        }
    }


    pub fn draw_text_z(&mut self, text: &str, pos: Vec3, scale: f32, default_colour: Vec4) {
        let mut x;
        let mut y = pos.y;
        let mut active_colour = default_colour;

        for l in text.lines() {
            x = pos.x;
            y += self.line_size * scale;


            let mut line_size = 0.0f32;
            let mut iter = l.chars();
            while let Some(c) = iter.next() {
                if c == '§' {
                    let colour_code = iter.next().unwrap();

                    active_colour = match colour_code {
                        '0' => Vec4::ZERO,
                        '1' => Vec4::new(0.0, 0.0, 0.4, 1.0),
                        '2' => Vec4::new(0.0, 0.4, 0.0, 1.0),
                        '3' => Vec4::new(0.0, 0.4, 0.4, 1.0),
                        '4' => Vec4::new(0.4, 0.0, 0.0, 1.0),
                        '5' => Vec4::new(0.4, 0.0, 0.4, 1.0),
                        '6' => Vec4::new(1.0, 0.4, 0.0, 1.0),
                        '7' => Vec4::new(0.4, 0.4, 0.4, 1.0),
                        '8' => Vec4::new(0.1, 0.1, 0.1, 1.0),
                        '9' => Vec4::new(0.1, 0.1, 1.0, 1.0),
                        'a' => Vec4::new(0.1, 1.0, 0.1, 1.0),
                        'b' => Vec4::new(0.1, 1.0, 1.0, 1.0),
                        'c' => Vec4::new(1.0, 0.1, 0.1, 1.0),
                        'd' => Vec4::new(1.0, 0.1, 1.0, 1.0),
                        'e' => Vec4::new(1.0, 1.0, 0.5, 1.0),
                        'f' => Vec4::ONE,
                        'r' => default_colour,

                        _ => {
                            warn!("invalid colour code '§{}', resetting to default colour", colour_code);
                            default_colour
                        },
                    };
                    continue
                }

                let Some(ch) = self.characters.get(&c)
                else { warn!("[renderer] draw-text: character not registered '{c}'"); continue };

                let xpos = x + ch.bearing.x as f32 * scale;
                let ypos = y - (ch.size.y + ch.bearing.y) as f32 * scale * 0.5;
                x += (ch.advance >> 6) as f32 * scale;

                let w = ch.size.x as f32 * scale;
                let h = ch.size.y as f32 * scale;
                line_size = line_size.max(h);

                let dims = Vec2::new(w, h);
                self.draw_tex_rect(Vec2::new(xpos, ypos), dims, ch.texture, active_colour);
            }


        }
    }


    pub fn draw_rect(&mut self, pos: Vec2, dims: Vec2, colour: Vec4) {
        self.draw_tex_rect(pos, dims, self.white_texture, colour);
    }


    pub fn window_size(&self) -> Vec2 {
        let (w, h) = (self.config.width, self.config.height);
        Vec2::new(w as _, h as _) / self.ui_scale
    }



    pub fn with_z<F: FnOnce(&mut Self)>(&mut self, mut z: f32, f: F) {
        assert!(z >= UI_Z_MIN && z <= UI_Z_MAX);
        let len = self.rects.len();
        f(self);

        let item_count = self.rects.len() - len;

        if (z - item_count as f32 * UI_DELTA_Z) >= UI_Z_MIN {
            for item in self.rects[len..].iter_mut().rev() {
                item.z = Some(z);
                z -= UI_DELTA_Z;
            }
        } else {
            for item in self.rects[len..].iter_mut() {
                z += UI_DELTA_Z;
                item.z = Some(z);
            }
        }
    }


    pub fn draw_tex_rect(&mut self, pos: Vec2, dims: Vec2, tex: TextureId, modulate: Vec4) {
        if modulate.w == 0.0 { return };

        let rect = DrawRect {
            modulate,
            pos,
            dims,
            tex,
            z: None,
        };

        self.rects.push(rect);
    }


    pub fn draw_tex_rect_z(&mut self, pos: Vec3, dims: Vec2, tex: TextureId, modulate: Vec4) {
        let rect = DrawRect {
            modulate,
            pos: Vec2::new(pos.x, pos.y),
            dims,
            tex,
            z: Some(pos.z),
        };

        self.rects.push(rect);
    }


    pub fn draw_mesh(&mut self, mesh: MeshIndex, instance: MeshInstance) {
        if self.mesh_draws.len() <= mesh.usize() {
            self.mesh_draws.resize(mesh.usize()+1, vec![]);
        }

        self.mesh_draws[mesh].push(instance);
    }


    pub fn draw_item(&mut self, item_kind: ItemKind, mut instance: MeshInstance) {
        if let ItemKind::Structure(structure) = item_kind {
            let blocks = structure.blocks(CardinalDirection::North);
            let mut min = IVec3::MAX;
            let mut max = IVec3::MIN;

            for &block in blocks {
                min = min.min(block);
                max = max.max(block);
            }

            let size = (max - min).abs() + IVec3::ONE;
            let size = size.as_vec3().max_element();
            let (scale, rot, trans) = instance.model.to_scale_rotation_translation();
            let scale = scale / size;
            instance.model = Mat4::from_scale_rotation_translation(scale, rot, trans);
        }


        let mesh = self.assets.get_item(item_kind);
        self.draw_mesh(mesh, instance);
    }


    pub fn draw_item_icon(&mut self, item: ItemKind, pos: Vec2, dims: Vec2, modulate: Vec4) {
        let texture = self.assets.get_ico(item);
        self.draw_tex_rect(pos, dims, texture, modulate);
    }


    pub fn text_size(&self, str: &str, scale: f32) -> Vec2 {
        let mut y_size : f32 = 0.0;
        let mut x_size : f32 = 0.0;

        for l in str.lines() {
            y_size += self.line_size * scale;
            let mut local_x_size = 0.0;
            let mut skip_next = false;

            for c in l.chars() {
                if skip_next { skip_next = false; continue };

                if c == '§' {
                    skip_next = true;
                    continue
                }

                let Some(ch) = self.characters.get(&c)
                else { warn!("[renderer] text-size: character not registered '{c}'"); continue };
                local_x_size += (ch.advance >> 6) as f32 * scale;
            }

            x_size = x_size.max(local_x_size);
        }

        Vec2::new(x_size, y_size)
    }
}


#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug)]
pub struct UIVertex {
    position: Vec3,
    pad: f32,
    uv: Vec2,
    pad1: f32,
    pad2: f32,
    modulate: Vec4,
}


impl UIVertex {
    pub fn new(position: Vec2, uv: Vec2, modulate: Vec4, z: f32) -> Self {
        Self {
            position: Vec3::new(position.x, position.y, z),
            uv,
            modulate,
            pad: 0.0,
            pad1: 0.0,
            pad2: 0.0,
        }
    }


    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<UIVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: offset_of!(UIVertex, position) as _,
                    shader_location: 0,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x2,
                    offset: offset_of!(UIVertex, uv) as _,
                    shader_location: 1,
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: offset_of!(UIVertex, modulate) as _,
                    shader_location: 2,
                }
            ],
        }
    }
}


pub fn point_in_rect(point: Vec2, rect_pos: Vec2, rect_size: Vec2) -> bool {
    point.x >= rect_pos.x &&
    point.y >= rect_pos.y &&
    point.x <= rect_pos.x + rect_size.x &&
    point.y <= rect_pos.y + rect_size.y
}



#[derive(Debug, Clone, Copy)]
struct ScreenRect {
    pos: Vec2,
    size: Vec2,
}


impl ScreenRect {
    pub fn new() -> Self {
        Self {
            pos: Vec2::MAX,
            size: Vec2::ZERO,
        }
    }


    fn include(&mut self, sr: ScreenRect) {
        self.pos = self.pos.min(sr.pos);

        let other_corner = sr.pos + sr.size;
        let rect_size = other_corner - self.pos;
        self.size = self.size.max(rect_size);
    }
}


#[derive(Debug, Clone, Copy)]
pub struct Style {
    bg: Vec4,
    margin: Vec4,
    min: Vec2,
    fallback_pos: Vec2,
}


impl Style {
    pub fn new() -> Self {
        Self {
            bg: Vec4::ZERO,
            margin: Vec4::ZERO,
            min: Vec2::MIN,
            fallback_pos: Vec2::MAX,
        }
    }


    pub fn bg(mut self, bg: Vec4) -> Self {
        self.bg = bg;
        self
    }

    pub fn margin(mut self, margin: Vec4) -> Self {
        self.margin = margin;
        self
    }


    pub fn min(mut self, min_size: Vec2) -> Self {
        self.min = min_size;
        self
    }


    pub fn fallback_pos(mut self, pos: Vec2) -> Self {
        self.fallback_pos = pos;
        self
    }
}


pub struct DepthBuffer {
    pub view: wgpu::TextureView,
    pub format: wgpu::TextureFormat,
}

impl DepthBuffer {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, sample_count: u32) -> Self {
        let format = wgpu::TextureFormat::Depth32Float;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { view, format }
    }
}


pub fn create_multisampled_framebuffer(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: config.width,
        height: config.height,
        depth_or_array_layers: 1,
    };


    let multisampled_frame_descriptor = &wgpu::TextureDescriptor {
        size,
        mip_level_count: 1,
        sample_count: MSAA_SAMPLE_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        label: None,
        view_formats: &[],
    };

    device
        .create_texture(multisampled_frame_descriptor)
        .create_view(&wgpu::TextureViewDescriptor::default())
}


#[derive(Default, Debug)]
pub struct View {
    elements: Vec<Element>,
    direction: ViewDirection,
}


#[derive(Default, Debug)]
enum ViewDirection {
    #[default]
    None,

    V,
    H,

}


impl View {
    pub fn hstack<'a>(&'a mut self, lambda: impl FnOnce(&mut View)) -> Stack<'a> {
        let mut view = View::default();
        view.direction = ViewDirection::H;

        lambda(&mut view);

        Stack {
            parent: self,
            body: view,
        }
    }


    pub fn vstack<'a>(&'a mut self, lambda: impl FnOnce(&mut View)) -> Stack<'a> {
        let mut view = View::default();
        view.direction = ViewDirection::V;

        lambda(&mut view);

        Stack {
            parent: self,
            body: view,
        }
    }


    pub fn zstack<'a>(&'a mut self, lambda: impl FnOnce(&mut View)) -> Stack<'a> {
        let mut view = View::default();
        view.direction = ViewDirection::None;

        lambda(&mut view);

        Stack {
            parent: self,
            body: view,
        }
    }


    pub fn spacer(&mut self) {
        self.elements.push(Element::Rect(ElementRect { colour: Vec4::ZERO, kind: RectKind::Flexbox, min: Vec2::ZERO }));
    }


    pub fn rect<'a>(&'a mut self) -> Rect<'a> {
        Rect {
            parent: self,
            element_rect: ElementRect {
                colour: Vec4::ONE,
                kind: RectKind::Flexbox,
                min: Vec2::ZERO
            },
        }
    }


    pub fn text(&mut self, str: &str) {
        self.elements.push(Element::Text(str.to_string()));
    }


    pub fn calc_min_size(&self, renderer: &Renderer) -> Vec2 {
        let mut min_size = Vec2::ZERO;
        for element in &self.elements {
            let size = match element {
                Element::Rect(rect) => {
                    rect.min
                },

                Element::View(view) => {
                    view.calc_min_size(renderer)
                },

                Element::Text(str) => {
                    let size = renderer.text_size(str, 1.0);
                    size
                },
            };


            let d = match self.direction {
                ViewDirection::None => continue,
                ViewDirection::H => 0,
                ViewDirection::V => 1,
            };

            min_size[d] += size[d];
            let d = (d+1) % 2;
            min_size[d] = min_size[d].max(size[d]);
        }

        min_size
    }


    pub fn render(
        &self, renderer: &mut Renderer,
        given_size: Vec2,
        start_pos: Vec2, depth: u32
    ) -> Vec2 {


        let min_size = self.calc_min_size(renderer);
        let flex_boxes_count = self.elements.iter()
            .map(|x| if matches!(x, Element::Rect(ElementRect { kind: RectKind::Flexbox, .. })) { 1 } else { 0 })
            .sum::<u32>();
        let left_over_space = given_size - min_size;
        let spacer_size = left_over_space / flex_boxes_count as f32;


        let mut pos = start_pos;
        let mut curr_size = Vec2::ZERO;

        
        let d = match self.direction {
            ViewDirection::None => None,
            ViewDirection::H => Some(0),
            ViewDirection::V => Some(1),
        };

        
        for elem in &self.elements {
            let size = match elem {
                Element::Rect(rect) => {
                    if let Some(d) = d && let RectKind::Flexbox = rect.kind {
                        let mut size = rect.min;
                        size[d] = size[d].max(spacer_size[d]);
                        renderer.draw_rect(pos.xy(), size.xy(), rect.colour);
                        dbg!(rect.colour);

                        size

                    } else if let RectKind::Frame = rect.kind {
                        renderer.draw_rect(pos.xy(), rect.min.xy(), rect.colour);
                        rect.min

                    } else {
                        warn!("uhhh?");
                        continue;
                    }
                }


                Element::View(view) => {
                    let mut view_size = given_size - pos;
                    if let Some(d) = d && flex_boxes_count != 0 {
                        view_size[d] = view.calc_min_size(renderer)[d];
                    }

                    view.render(renderer, view_size, pos, depth)
                },


                Element::Text(text) => {
                    let size = renderer.text_size(text, 1.0);
                    renderer.draw_rect(pos.xy(), size, Vec4::ZERO);
                    renderer.draw_text(text, pos.xy(), 1.0, Vec4::ONE);
                    size
                },
            };


            renderer.draw_rect(pos.xy(), size.xy(), Vec4::new(0.5, 0.0, 0.0, 0.3));

            if let Some(d) = d {
                pos[d] += size[d];

                curr_size[d] = curr_size[d].max(size[d]);
                let d = (d+1) % 2;
                curr_size[d] = curr_size[d].max(size[d]);
            } else {
                curr_size = curr_size.max(size);
            }
        }


        if let Some(d) = d && flex_boxes_count != 0 {
            curr_size[d] = given_size[d];
        }



        renderer.draw_rect(start_pos.xy(), curr_size.xy(), Vec4::new(0.0, 0.1, 0.0, 0.1));
        curr_size
    }
}


#[derive(Debug)]
enum Element {
    Rect(ElementRect),

    View(View),

    Text(String),
}


pub struct Rect<'a> {
    parent: &'a mut View,

    element_rect: ElementRect,
}



#[derive(Default, Debug)]
pub struct ElementRect {
    colour: Vec4,
    kind: RectKind,
    min: Vec2,
}


#[derive(Default, Debug)]
pub enum RectKind {
    #[default]
    Frame,
    Flexbox,
}


impl<'a> Drop for Rect<'a> {
    fn drop(&mut self) {
        self.parent.elements.push(Element::Rect(core::mem::take(&mut self.element_rect)));
    }
}


impl<'a> Deref for Rect<'a> {
    type Target = ElementRect;


    fn deref(&self) -> &Self::Target {
        &self.element_rect
    }

}


impl<'a> DerefMut for Rect<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.element_rect
    }
}


pub struct VStack<'a> {
    parent: &'a mut View,
    body: View,

}


impl<'a> Drop for VStack<'a> {
    fn drop(&mut self) {
        self.parent.elements.push(Element::View(core::mem::take(&mut self.body)));
    }
}



pub struct Stack<'a> {
    parent: &'a mut View,
    body: View,

}


impl<'a> Drop for Stack<'a> {
    fn drop(&mut self) {
        self.parent.elements.push(Element::View(core::mem::take(&mut self.body)));
    }
}


