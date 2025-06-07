pub mod textures;

use std::{collections::HashMap, ffi::CString, fs, mem::offset_of, ptr::null_mut};

use freetype::freetype::{FT_Done_Face, FT_Done_FreeType, FT_Load_Char, FT_Set_Pixel_Sizes, FT_LOAD_RENDER};
use glam::{IVec2, Mat4, Quat, Vec2, Vec3, Vec4, Vec4Swizzles};
use glfw::{ffi::glfwGetProcAddress, Context, GlfwReceiver, PWindow, WindowEvent};
use textures::{TextureAtlasBuilder, TextureAtlasManager, TextureId};

use crate::{items::{ItemKind, Assets}, shader::{Shader, ShaderProgram, ShaderType}};


const FONT_SIZE : u32 = 64;


// the renderer is done,
// never to be touched until the heat death of the universe
// ..or shadows need casting
// whichever comes first

pub struct Renderer {
    pub glfw: glfw::Glfw,
    pub window: PWindow,
    pub window_events: GlfwReceiver<(f64, WindowEvent)>,

    pub quad_vao: u32,
    pub quad_vbo: u32,
    pub white: TextureId,

    pub characters: HashMap<char, Character>,
    pub biggest_y_size: f32,

    pub is_wireframe: bool,

    pub atlases: TextureAtlasManager,

    pub meshes: Assets,
    pub z: f32,
    pub ui_scale: f32,
    pub rects: Vec<DrawRect>,

    current_rect: ScreenRect,
}


pub struct DrawRect {
    modulate: Vec4,
    pos: Vec2,
    dims: Vec2,
    tex: TextureId,
}


#[derive(Debug)]
pub struct Character {
    pub texture: TextureId,
    pub size: IVec2,
    pub bearing: IVec2,
    pub advance: u32,
}


impl Renderer {
    pub fn new(window_size: (usize, usize)) -> Self {

        let mut glfw = glfw::init(|error, str| println!("[error] glfw error {str}: {error}"))
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
                    println!("[warn] failed to load gl function '{s}'");
                }
                result
            });
        }

        let fragment = Shader::new(&fs::read("text.fs").unwrap(), ShaderType::Fragment).unwrap();
        let vertex = Shader::new(&fs::read("text.vs").unwrap(), ShaderType::Vertex).unwrap();
        let text_shader = ShaderProgram::new(fragment, vertex).unwrap();

        let fragment = Shader::new(&fs::read("ui.fs").unwrap(), ShaderType::Fragment).unwrap();
        let vertex = Shader::new(&fs::read("ui.vs").unwrap(), ShaderType::Vertex).unwrap();
        let ui_shader = ShaderProgram::new(fragment, vertex).unwrap();

        unsafe { gl::Enable(gl::DEPTH_TEST) };
        unsafe { gl::Enable(gl::BLEND) };
        unsafe { gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA); };




        let mut ft = null_mut();
        if unsafe { freetype::freetype::FT_Init_FreeType(&mut ft) } != 0 {
            panic!("[error] failed to init freetype library");
        }


        let mut face = null_mut();
        if unsafe { freetype::freetype::FT_New_Face(ft, c"font.ttf".as_ptr(), 0, &mut face) } != 0 {
            panic!("[error] failed to load font");
        }


        dbg!(unsafe { FT_Set_Pixel_Sizes(face, FONT_SIZE, FONT_SIZE) });


        let mut characters = HashMap::new();
        let mut texture_atlas = TextureAtlasBuilder::new(GpuTextureFormat::Red);

        unsafe { gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1) };

        let mut biggest_y_size : f32 = 0.0;
        for c in 0..128 {
            if unsafe { FT_Load_Char(face, c as u64, FT_LOAD_RENDER as _) } != 0 {
                panic!("[error] failed to load glyph '{}'", char::from_u32(c).unwrap());
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

        let this = Self {
            glfw,
            window,
            window_events,
            quad_vao,
            quad_vbo,
            white,
            characters,
            is_wireframe: false,
            biggest_y_size,
            meshes: assets,
            atlases,
            z: 1.0,
            ui_scale: 1.0,
            rects: vec![],
            current_rect: ScreenRect::new(),
        };

        this
    }


    pub fn begin(&mut self) {
        self.z = 1.0;
        unsafe {
            gl::Enable(gl::DEPTH_TEST);
            gl::ClearColor(0.05, 0.05, 0.05, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);


            if self.is_wireframe {
                gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE);
            } else {
                gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL);
            }
        }
    }


    pub fn end(&mut self) {
        unsafe { gl::Clear(gl::DEPTH_BUFFER_BIT) };

        for rect in self.rects.iter() {
            let tex = rect.tex;
            let pos = rect.pos;
            let dims = rect.dims;
            let modulate = rect.modulate;

            let uvs = self.atlases.get_uv(tex);
            let buf = self.atlases.buf(tex);

            let x0 = uvs.x;
            let y0 = uvs.y;
            let x1 = uvs.z;
            let y1 = uvs.w;

            buf.push(UIVertex::new(pos+Vec2::new(0.0, dims.y), Vec2::new(x0, y1), modulate, self.z));
            buf.push(UIVertex::new(pos, Vec2::new(x0, y0), modulate, self.z));
            buf.push(UIVertex::new(pos+Vec2::new(dims.x, 0.0), Vec2::new(x1, y0), modulate, self.z));

            buf.push(UIVertex::new(pos+Vec2::new(0.0, dims.y), Vec2::new(x0, y1), modulate, self.z));
            buf.push(UIVertex::new(pos+Vec2::new(dims.x, 0.0), Vec2::new(x1, y0), modulate, self.z));
            buf.push(UIVertex::new(pos+dims, Vec2::new(x1, y1), modulate, self.z));

            self.z += 0.0001;
        }
        self.rects.clear();

        let projection = glam::Mat4::orthographic_rh(0.0, self.window.get_size().0 as f32 / self.ui_scale, self.window.get_size().1 as f32 / self.ui_scale, 0.0, 0.001, 100.0);
        for (atlas, shader, buf) in self.atlases.atlases.values_mut() {
            shader.use_program();
            shader.set_matrix4(c"projection", projection);
            unsafe {
                // update buffer
                gl::BindVertexArray(self.quad_vao);
                gl::BindBuffer(gl::ARRAY_BUFFER, self.quad_vbo);
                gl::BufferData(gl::ARRAY_BUFFER, (size_of::<UIVertex>() * buf.len()) as _,
                                buf.as_ptr() as _, gl::DYNAMIC_DRAW);

                // render
                gl::ActiveTexture(gl::TEXTURE0);
                gl::BindTexture(gl::TEXTURE_2D, atlas.gpu_texture.id);

                gl::DrawArrays(gl::TRIANGLES, 0, buf.len() as _);
            }

            buf.clear();
        }

        self.window.swap_buffers();
        self.glfw.poll_events();
    }


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
                            println!("[warn] invalid colour code '§{}', resetting to default colour", colour_code);
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


    pub fn with_style<F: FnOnce(&mut Self)>(&mut self, style: Style, f: F) {
        let mut prev_rect = self.current_rect;
        self.current_rect = ScreenRect::new();
        let len = self.rects.len();

        f(self);

        dbg!(self.current_rect);
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
        };

        self.rects.push(rect);
        self.current_rect.include(ScreenRect { pos, size: dims });
    }


    pub fn draw_item(&self, shader: &ShaderProgram, item_kind: ItemKind, pos: Vec3, scale: Vec3, rot: f32) {
        let model = Mat4::from_scale_rotation_translation(scale, Quat::from_rotation_y(rot), pos);
        shader.set_matrix4(c"model", model);

        self.meshes.get(item_kind).draw();
    }


    pub fn draw_item_icon(&mut self, item: ItemKind, pos: Vec2, dims: Vec2, modulate: Vec4) {
        let texture = self.meshes.get_ico(item);
        self.draw_tex_rect(pos, dims, texture, modulate);
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
    }
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
