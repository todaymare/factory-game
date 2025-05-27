use std::{collections::HashMap, ffi::CString, fs, ptr::null_mut};

use freetype::freetype::{FT_Done_Face, FT_Done_FreeType, FT_Load_Char, FT_Set_Pixel_Sizes, FT_LOAD_RENDER};
use glam::{IVec2, Mat4, Quat, Vec2, Vec3};
use glfw::{ffi::glfwGetProcAddress, Context, GlfwReceiver, PWindow, WindowEvent};
use sti::println;

use crate::{items::{ItemKind, ItemMeshes}, shader::{Shader, ShaderProgram, ShaderType}, TICKS_PER_SECOND};


// the renderer is done,
// never to be touched until the heat death of the universe
// ..or shadows need casting
// whichever comes first

pub struct Renderer {
    pub glfw: glfw::Glfw,
    pub window: PWindow,
    pub window_events: GlfwReceiver<(f64, WindowEvent)>,

    pub text_shader: ShaderProgram,

    pub quad_vao: u32,
    pub quad_vbo: u32,
    pub quad_tex: u32,

    pub characters: HashMap<char, Character>,
    pub biggest_y_size: f32,

    pub is_wireframe: bool,

    pub meshes: ItemMeshes,
}


#[derive(Debug)]
pub struct Character {
    pub texture_id: u32,
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


        unsafe { FT_Set_Pixel_Sizes(face, 0, 48) };


        let mut characters = HashMap::new();

        unsafe { gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1) };

        let mut biggest_y_size : f32 = 0.0;
        for c in 0..128 {
            if unsafe { FT_Load_Char(face, c as u64, FT_LOAD_RENDER as _) } != 0 {
                panic!("[error] failed to load glyph '{}'", char::from_u32(c).unwrap());
            }


            let mut texture = 0;
            unsafe {
                gl::GenTextures(1, &mut texture);
                gl::BindTexture(gl::TEXTURE_2D, texture);
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    gl::RED as _,
                    (*(*face).glyph).bitmap.width as _,
                    (*(*face).glyph).bitmap.rows as _,
                    0,
                    gl::RED,
                    gl::UNSIGNED_BYTE,
                    (*(*face).glyph).bitmap.buffer as _,
                );

                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_BORDER as _);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_BORDER as _);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _);

                let character = Character {
                    texture_id: texture,
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
            gl::BufferData(gl::ARRAY_BUFFER, size_of::<f32>() as isize * 6 * 4, null_mut(), gl::STATIC_DRAW);

            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(0, 4, gl::FLOAT, gl::FALSE, 4 * size_of::<f32>() as i32, null_mut());

            gl::BindVertexArray(0);
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);

            (vao, vbo)
        };



        let verticies : [[f32; 4]; 6] = [
            [ 0.0, 1.0,   0.0, 1.0 ],
            [ 0.0, 0.0,   0.0, 0.0 ],
            [ 1.0, 0.0,   1.0, 0.0 ],

            [ 0.0, 1.0,   0.0, 1.0 ],
            [ 1.0, 0.0,   1.0, 0.0 ],
            [ 1.0, 1.0,   1.0, 1.0 ],
        ];

        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, quad_vbo);
            gl::BufferData(gl::ARRAY_BUFFER, size_of_val(&verticies) as isize, verticies.as_ptr().cast(), gl::STATIC_DRAW);
        }


        let mut this = Self {
            glfw,
            window,
            window_events,
            text_shader,
            quad_vao,
            quad_vbo,
            quad_tex: quad_texture,
            characters,
            is_wireframe: false,
            biggest_y_size,
            meshes: ItemMeshes::new(),
        };

        this.resize();
        this
    }


    pub fn resize(&mut self) {
        let projection = glam::Mat4::orthographic_rh(0.0, self.window.get_size().0 as f32, self.window.get_size().1 as f32, 0.0, 0.001, 100.0);
        self.text_shader.use_program();
        self.text_shader.set_matrix4(c"projection", projection);
    }


    pub fn begin(&self) {
        unsafe {
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
        self.window.swap_buffers();
        self.glfw.poll_events();
    }


    pub fn draw_text(&self, text: &str, pos: Vec2, scale: f32, default_colour: Vec3) {
        if self.is_wireframe {
            //unsafe { gl::PolygonMode(gl::FRONT_AND_BACK, gl::FILL); }
        }

        self.text_shader.use_program();
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindVertexArray(self.quad_vao);

            let mut x;
            let mut y = pos.y;
            let mut active_colour = default_colour;

            self.text_shader.set_vec3(c"textColor", active_colour);

            for l in text.lines() {
                y += self.biggest_y_size * scale;
                x = pos.x;


                let mut iter = l.chars();
                while let Some(c) = iter.next() {
                    if c == '§' {
                        let colour_code = iter.next().unwrap();

                        active_colour = match colour_code {
                            '0' => Vec3::ZERO,
                            '1' => Vec3::new(0.0, 0.0, 0.4),
                            '2' => Vec3::new(0.0, 0.4, 0.0),
                            '3' => Vec3::new(0.0, 0.4, 0.4),
                            '4' => Vec3::new(0.4, 0.0, 0.0),
                            '5' => Vec3::new(0.4, 0.0, 0.4),
                            '6' => Vec3::new(1.0, 0.4, 0.0),
                            '7' => Vec3::new(0.4, 0.4, 0.4),
                            '8' => Vec3::new(0.1, 0.1, 0.1),
                            '9' => Vec3::new(0.1, 0.1, 1.0),
                            'a' => Vec3::new(0.1, 1.0, 0.1),
                            'b' => Vec3::new(0.1, 1.0, 1.0),
                            'c' => Vec3::new(1.0, 0.1, 0.1),
                            'd' => Vec3::new(1.0, 0.1, 1.0),
                            'e' => Vec3::new(1.0, 1.0, 0.7),
                            'f' => Vec3::ONE,
                            'r' => default_colour,

                            _ => {
                                println!("[warn] invalid colour code '§{}', resetting to default colour", colour_code);
                                default_colour
                            },
                        };

                        self.text_shader.set_vec3(c"textColor", active_colour);

                        continue
                    }
                    let ch = self.characters.get(&c).unwrap();

                    let xpos = x + ch.bearing.x as f32 * scale;
                    let ypos = y - (ch.size.y + ch.bearing.y) as f32 * scale * 0.5;

                    let w = ch.size.x as f32 * scale;
                    let h = ch.size.y as f32 * scale;

                    let model = Mat4::from_scale_rotation_translation(Vec3::new(w, h, 1.0), Quat::IDENTITY, Vec3::new(xpos, ypos, 1.0));

                    gl::BindTexture(gl::TEXTURE_2D, ch.texture_id);
                    self.text_shader.set_matrix4(c"model", model);

                    gl::BindBuffer(gl::ARRAY_BUFFER, self.quad_vbo);

                    gl::DrawArrays(gl::TRIANGLES, 0, 6);

                    x += (ch.advance >> 6) as f32 * scale;
                }


            }
        }

        if self.is_wireframe {
            unsafe { gl::PolygonMode(gl::FRONT_AND_BACK, gl::LINE); }
        }
    }


    pub fn draw_rect(&self, pos: Vec2, dims: Vec2, colour: Vec3) {

        unsafe {
            gl::Uniform3f(gl::GetUniformLocation(self.text_shader.id, c"textColor".as_ptr()), colour.x, colour.y, colour.z);

            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.quad_tex);
            gl::BindVertexArray(self.quad_vao);

            let model = Mat4::from_scale_rotation_translation(Vec3::new(dims.x, dims.y, 1.0), Quat::IDENTITY, Vec3::new(pos.x, pos.y, 1.0));
            self.text_shader.set_matrix4(c"model", model);

            gl::BindBuffer(gl::ARRAY_BUFFER, self.quad_vbo);
            gl::DrawArrays(gl::TRIANGLES, 0, 6);
        }
    }


    pub fn draw_item(&self, shader: &ShaderProgram, item_kind: ItemKind, pos: Vec3, scale: Vec3, rot: f32) {
        let model = Mat4::from_scale_rotation_translation(scale, Quat::from_rotation_y(rot), pos);
        shader.set_matrix4(c"model", model);

        self.meshes.get(item_kind).draw();
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


