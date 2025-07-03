pub mod textures;
pub mod uniform;
pub mod ssbo;
pub mod gpu_allocator;

use std::cell::Cell;

use bytemuck::{Pod, Zeroable};
use glam::{IVec2, IVec3, Mat4, Vec2, Vec3, Vec4};
use gpu_allocator::GPUAllocator;
use ssbo::{ResizableBuffer, SSBO};
use sti::{define_key, static_assert_eq};
use textures::TextureId;
use uniform::Uniform;
use wgpu::{util::{BufferInitDescriptor, DeviceExt, StagingBelt}, wgt::DrawIndexedIndirectArgs, BufferUsages, TextureUsages, *};
use winit::window::Window;

use crate::{free_list::FreeKVec, voxel_world::mesh::{ChunkMeshFramedata, ChunkQuadInstance}, constants::{QUAD_INDICES, QUAD_VERTICES}};


// the renderer is done,
// never to be touched until the heat death of the universe
// ..or shadows need casting
// whichever comes first

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub voxel_pipeline: VoxelPipeline,
    pub staging_buffer: StagingBelt,


    pub ui_scale: f32,
    pub rects: Vec<DrawRect>,

    pub draw_count: Cell<u32>,
    pub triangle_count: Cell<u32>,
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


#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct MeshShaderUniform {
    model: Mat4,
    modulate: Vec4,
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


define_key!(pub MeshIndex(u32));


pub struct VoxelPipeline {
    pub pipeline: RenderPipeline,
    pub frame_uniform: Uniform<VoxelShaderUniform>,
    pub model_uniform: SSBO<ChunkMeshFramedata>,
    pub depth_buffer: DepthBuffer,

    pub chunk_offsets: FreeKVec<MeshIndex, ChunkMeshFramedata>,
    pub instances: GPUAllocator<ChunkQuadInstance>,
    pub indirect_buf: ResizableBuffer<DrawIndexedIndirectArgs>,
    pub vertex_buf: Buffer,
    pub index_buf: Buffer,
}


impl Renderer {
    pub async fn new(window: &'static Window) -> Self {

        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

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
                    limits.max_buffer_size = 17179869184;
                    limits.max_storage_buffer_binding_size = 512 << 20;
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


        let voxel_pipeline = {
            let voxel_shader_uniform = Uniform::<VoxelShaderUniform>::new("voxel-shader-frame-uniform", &device, 0, ShaderStages::VERTEX_FRAGMENT);
            let ssbo = SSBO::new("voxel-shader-chunk-offsets-ssbo", &device, BufferUsages::COPY_DST | BufferUsages::COPY_SRC | BufferUsages::STORAGE, 16 * 1024 * 400);

            let shader = device.create_shader_module(
                wgpu::ShaderModuleDescriptor {
                    label: Some("voxel-shader"),
                    source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/voxel.wgsl").into()),
                }
            );


            let rpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("voxel-render-pipeline-layout"),
                bind_group_layouts: &[voxel_shader_uniform.bind_group_layout(), ssbo.layout()],
                push_constant_ranges: &[],
            });


            let depth_texture = DepthBuffer::new(&device, config.width, config.height); 


            let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("voxel-render-pipeline"),
                layout: Some(&rpl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"), // 1.
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: 12,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Sint32x3,
                                    offset: 0,
                                    shader_location: 0,
                                }
                            ],
                        },
                        ChunkQuadInstance::desc(),
                    ], 
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState { // 3.
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState { // 4.
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
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
                    count: 1, // 2.
                    mask: !0, // 3.
                    alpha_to_coverage_enabled: false, // 4.
                },
                multiview: None, // 5.
                cache: None, // 6.
            });


            let vertex = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("quad-vertices"),
                    usage: BufferUsages::VERTEX,
                    contents: bytemuck::cast_slice(QUAD_VERTICES),
                });

            let indices = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("quad-indices"),
                    usage: BufferUsages::INDEX,
                    contents: bytemuck::cast_slice(QUAD_INDICES),
                });


            let indirect = ResizableBuffer::new(
                "voxel-shader-indirect-buffer",
                &device,
                BufferUsages::INDIRECT | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                1024
            );


            VoxelPipeline {
                pipeline: render_pipeline,
                frame_uniform: voxel_shader_uniform,
                model_uniform: ssbo,
                depth_buffer: depth_texture,
                vertex_buf: vertex,
                index_buf: indices,
                instances: GPUAllocator::new(&device, 1),
                indirect_buf: indirect,
                chunk_offsets: FreeKVec::new(),
            }
        };

        /*
        let mut glfw = glfw::init(|error, str| error!("glfw error {str}: {error}"))
            .unwrap();

        glfw.window_hint(glfw::WindowHint::ContextVersion(3, 3));
        glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
        glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));

        let (mut window, window_events) = glfw.create_window(window_size.0 as u32, window_size.1 as u32, "factory game", glfw::WindowMode::Windowed)
            .unwrap();

        window.set_all_polling(true);

        glfw.make_context_current(Some(&window));
        unsafe {
            gl::load_with(|s| {
                let cstr = CString::new(s).unwrap();
                let result = glfwGetProcAddress(cstr.as_ptr());

                if result.is_null() {
                    warn!("failed to load gl function '{s}'");
                }
                result
            });
        }

        let fragment = Shader::new(&fs::read("shaders/text.fs").unwrap(), ShaderType::Fragment).unwrap();
        let vertex = Shader::new(&fs::read("shaders/text.vs").unwrap(), ShaderType::Vertex).unwrap();
        let text_shader = ShaderProgram::new(fragment, vertex).unwrap();

        let fragment = Shader::new(&fs::read("shaders/ui.fs").unwrap(), ShaderType::Fragment).unwrap();
        let vertex = Shader::new(&fs::read("shaders/ui.vs").unwrap(), ShaderType::Vertex).unwrap();
        let ui_shader = ShaderProgram::new(fragment, vertex).unwrap();

        //unsafe { gl::Enable(gl::DEPTH_TEST) };
        unsafe { gl::Enable(gl::BLEND) };
        unsafe { gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA); };

        let mut ft = null_mut();
        if unsafe { freetype::freetype::FT_Init_FreeType(&mut ft) } != 0 {
            panic!("failed to init freetype library");
        }


        let mut face = null_mut();
        if unsafe { freetype::freetype::FT_New_Face(ft, c"font.ttf".as_ptr(), 0, &mut face) } != 0 {
            panic!("failed to load font");
        }

        unsafe { FT_Set_Pixel_Sizes(face, FONT_SIZE, FONT_SIZE) };

        let mut characters = HashMap::new();
        let mut texture_atlas = TextureAtlasBuilder::new(GpuTextureFormat::Red);

        unsafe { gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1) };

        let mut biggest_y_size : f32 = 0.0;
        for c in 0..128 {
            if unsafe { FT_Load_Char(face, c as u64, FT_LOAD_RENDER as _) } != 0 {
                panic!("failed to load glyph '{}'", char::from_u32(c).unwrap());
            }


            unsafe {
                let dims = IVec2::new(
                    (*(*face).glyph).bitmap.width as _,
                    (*(*face).glyph).bitmap.rows as _,
                );

                let slice = if dims != IVec2::ZERO { core::slice::from_raw_parts((*(*face).glyph).bitmap.buffer, (dims.x * dims.y) as usize) }
                            else { &[] };
                let texture = texture_atlas.register(dims, slice);

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
                biggest_y_size = biggest_y_size.max(h);

                characters.insert(char::from_u32(c).unwrap(), character);
            }
        }

        let white = texture_atlas.register(IVec2::new(32, 32), &[255; 32*32]);
        let font_ta = texture_atlas.build();

        unsafe {
            FT_Done_Face(face);
            FT_Done_FreeType(ft);
        }


        let mut quad_texture = 0;
        unsafe {
            gl::GenTextures(1, &mut quad_texture);
            gl::BindTexture(gl::TEXTURE_2D, quad_texture);

            gl::TexImage2D(gl::TEXTURE_2D,
                           0,
                           gl::RED as _,
                           1, 
                           1,
                           0,
                           gl::RED,
                           gl::UNSIGNED_BYTE,
                           (&[255u8]).as_ptr().cast());
        }


     
        let (quad_vao, quad_vbo) = unsafe {
            let mut vao = 0;
            let mut vbo = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            gl::BindVertexArray(vao);

            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);

            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(0, 3,
                                    gl::FLOAT, gl::FALSE, size_of::<UIVertex>() as i32,
                                    offset_of!(UIVertex, position) as _);


            gl::EnableVertexAttribArray(1);
            gl::VertexAttribPointer(1, 2,
                                    gl::FLOAT, gl::FALSE, size_of::<UIVertex>() as i32,
                                    offset_of!(UIVertex, uv) as _);


            gl::EnableVertexAttribArray(2);
            gl::VertexAttribPointer(2, 4,
                                    gl::FLOAT, gl::FALSE, size_of::<UIVertex>() as i32,
                                    offset_of!(UIVertex, modulate) as _);

            gl::BindVertexArray(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);

            (vao, vbo)
        };

        let mut assets_ta = TextureAtlasBuilder::new(GpuTextureFormat::RGBA);
        let assets = Assets::new(&mut assets_ta);

        let mut atlases = TextureAtlasManager::new();
        atlases.register(assets_ta.build(), ui_shader);
        atlases.register(font_ta, text_shader);

        glfw.set_swap_interval(glfw::SwapInterval::None);
*/
        let this = Self {
            ui_scale: 1.0,
            rects: vec![],
            draw_count: Cell::new(0),
            triangle_count: Cell::new(0),
            surface,
            device,
            queue,
            config,
            //mesh_pipeline,
            voxel_pipeline,
            staging_buffer: StagingBelt::new(128 << 20),
        };

        this
    }


    /*
    pub fn to_point(&self, pos: Vec2) -> Vec2 {
        pos / self.ui_scale
    }


    pub fn draw_text(&mut self, text: &str, pos: Vec2, scale: f32, default_colour: Vec4) {
        let mut x;
        let mut y = pos.y;
        let mut active_colour = default_colour;

        for l in text.lines() {
            y += self.biggest_y_size * scale;
            x = pos.x;


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
                        'e' => Vec4::new(1.0, 1.0, 0.7, 1.0),
                        'f' => Vec4::ONE,
                        'r' => default_colour,

                        _ => {
                            warn!("invalid colour code '§{}', resetting to default colour", colour_code);
                            default_colour
                        },
                    };
                    continue
                }
                let ch = self.characters.get(&c).unwrap();

                let xpos = x + ch.bearing.x as f32 * scale;
                let ypos = y - (ch.size.y + ch.bearing.y) as f32 * scale * 0.5;
                x += (ch.advance >> 6) as f32 * scale;

                let w = ch.size.x as f32 * scale;
                let h = ch.size.y as f32 * scale;

                let dims = Vec2::new(w, h);
                self.draw_tex_rect(Vec2::new(xpos, ypos), dims, ch.texture, active_colour);
            }


        }
    }


    pub fn draw_rect(&mut self, pos: Vec2, dims: Vec2, colour: Vec4) {
        self.draw_tex_rect(pos, dims, self.white, colour);
    }


    pub fn window_size(&self) -> Vec2 {
        let (w, h) = self.window.get_size();
        Vec2::new(w as _, h as _) / self.ui_scale
    }



    pub fn with_z<F: FnOnce(&mut Self)>(&mut self, mut z: f32, f: F) {
        let len = self.rects.len();
        f(self);

        for item in &mut self.rects[len..] {
            item.z = Some(z);
            z += 0.0001;
        }
    }


    pub fn with_style<F: FnOnce(&mut Self)>(&mut self, style: Style, f: F) {
        let mut prev_rect = self.current_rect;
        self.current_rect = ScreenRect::new();
        let len = self.rects.len();

        f(self);

        self.current_rect.pos = self.current_rect.pos.min(style.fallback_pos);
        self.current_rect.size = self.current_rect.size.max(style.min);


        if style.margin != Vec4::ZERO {
            self.current_rect.pos -= style.margin.xy();
            self.current_rect.size += style.margin.xy() + style.margin.zw();
        }

        if style.bg != Vec4::ZERO {
            let rect = DrawRect {
                modulate: style.bg,
                pos: self.current_rect.pos,
                dims: self.current_rect.size,
                tex: self.white,
                z: None,
            };
            self.rects.insert(len, rect);
        }

        prev_rect.include(self.current_rect);
        self.current_rect = prev_rect;

    }

    pub fn draw_tex_rect(&mut self, pos: Vec2, dims: Vec2, tex: TextureId, modulate: Vec4) {
        let rect = DrawRect {
            modulate,
            pos,
            dims,
            tex,
            z: None,
        };

        self.rects.push(rect);
        self.current_rect.include(ScreenRect { pos, size: dims });
    }


    pub fn draw_item_quat(&self, shader: &ShaderProgram, item_kind: ItemKind, pos: Vec3, mut scale: Vec3, rot: Quat) {
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
            scale /= size.abs();
        }


        let model = Mat4::from_scale_rotation_translation(scale, rot, pos);
        shader.set_matrix4(c"model", model);

        let mesh = self.meshes.get(item_kind);
        self.draw_mesh(mesh);
    }


    pub fn draw_item(&self, shader: &ShaderProgram, item_kind: ItemKind, pos: Vec3, scale: Vec3, rot: Vec3) {
        let rot = Quat::from_euler(glam::EulerRot::XYZ, rot.x, rot.y, rot.z);
        self.draw_item_quat(shader, item_kind, pos, scale, rot);
    }



    pub fn draw_item_icon(&mut self, item: ItemKind, pos: Vec2, dims: Vec2, modulate: Vec4) {
        let texture = self.meshes.get_ico(item);
        self.draw_tex_rect(pos, dims, texture, modulate);
    }


    pub fn draw_mesh(&self, mesh: &Mesh) {
        let this = &mesh;
        unsafe {
            gl::BindVertexArray(this.vao);
            gl::DrawElements(gl::TRIANGLES, this.indices as _, gl::UNSIGNED_INT, null_mut());
            gl::BindVertexArray(0);
        }

        self.triangle_count.set(self.triangle_count.get() + mesh.indices);
        self.draw_count.set(self.draw_count.get() + 1);
    }


    pub fn draw_voxel_mesh(&self, mesh: &ChunkMesh) {
        let this = &mesh;
        unsafe {
            gl::BindVertexArray(this.vao);
            gl::DrawElements(gl::TRIANGLES, this.indices as _, gl::UNSIGNED_INT, null_mut());
            gl::BindVertexArray(0);
        }

        self.triangle_count.set(self.triangle_count.get() + mesh.indices);
        self.draw_count.set(self.draw_count.get() + 1);
    }


    pub fn text_size(&self, str: &str, scale: f32) -> Vec2 {
        let mut y_size : f32 = 0.0;
        let mut x_size : f32 = 0.0;

        for l in str.lines() {
            y_size += self.biggest_y_size * scale;
            let mut local_x_size = 0.0;

            for c in l.chars() {
                let ch = self.characters.get(&c).unwrap();
                local_x_size += (ch.advance >> 6) as f32 * scale;
            }

            x_size = x_size.max(local_x_size);
        }

        Vec2::new(x_size, y_size)
    }*/
}


#[repr(C)]
pub struct UIVertex {
    position: Vec3,
    uv: Vec2,
    modulate: Vec4,
}


impl UIVertex {
    pub fn new(position: Vec2, uv: Vec2, modulate: Vec4, z: f32) -> Self {
        Self {
            position: Vec3::new(position.x, position.y, z),
            uv,
            modulate,
        }
    }
}


#[derive(Debug)]
pub struct GpuTexture {
    id: u32,
    format: GpuTextureFormat,
}


impl GpuTexture {
    pub fn new(format: GpuTextureFormat) -> Self {
        let mut id = 0;
        unsafe {
            gl::GenTextures(1, &mut id);

            gl::BindTexture(gl::TEXTURE_2D, id);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_BORDER as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_BORDER as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);


        }

        GpuTexture { id, format }
    }


    pub fn set_data(&self, dims: IVec2, data: &[u8]) {
        self.set_data_raw(dims, data.as_ptr());
    }


    pub fn set_data_raw(&self, dims: IVec2, data: *const u8) {
        unsafe {
        let (format, typ) = match self.format {
            GpuTextureFormat::Red => (gl::RED, gl::UNSIGNED_BYTE),
            GpuTextureFormat::RGBA => (gl::RGBA, gl::UNSIGNED_BYTE),
        };

        gl::BindTexture(gl::TEXTURE_2D, self.id);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            format as _,
            dims.x,
            dims.y,
            0,
            format,
            typ,
            data as _
        );

        }
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GpuTextureFormat {
    Red,
    RGBA,
}
impl GpuTextureFormat {
    fn pixel_size(&self) -> u32 {
        match self {
            GpuTextureFormat::Red => 1,
            GpuTextureFormat::RGBA => 4,
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
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let format = wgpu::TextureFormat::Depth32Float;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { view, format }
    }
}
