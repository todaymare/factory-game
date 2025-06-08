use std::{ffi::CStr, ptr::null_mut};

use glam::{Mat4, Vec3, Vec4};


pub struct Shader {
    id: u32
}


#[derive(Debug)]
pub enum ShaderError {
    InputIsTooBig(usize),
    CompileError(String),
}


#[derive(Debug)]
pub enum ShaderType {
    Fragment,
    Vertex,
}


#[derive(Clone)]
pub struct ShaderProgram {
    pub id: u32,
}


#[derive(Debug)]
pub struct ShaderProgramError {
    info: String
}


impl Shader {
    /// 
    /// # Assumes:
    /// - OpenGL functions are loaded
    ///
    pub fn new(data: &[u8], shader_type: ShaderType) -> Result<Self, ShaderError> {
        println!("[info] loading a {shader_type:?} shader");
        let id = {
            let shader = match shader_type {
                ShaderType::Fragment => gl::FRAGMENT_SHADER,
                ShaderType::Vertex => gl::VERTEX_SHADER,
            };

            unsafe { gl::CreateShader(shader) }
        };

        // id == 0, means an invalid id was passed to create shader
        // which shouldn't be possible
        debug_assert_ne!(id, 0);

        let Ok(len) : Result<i32, _> = data.len().try_into()
        else { return Err(ShaderError::InputIsTooBig(data.len())) };

        // Load the shader source
        unsafe { gl::ShaderSource(id, 1, &data.as_ptr().cast(), &len) };

        // Compile the shader
        unsafe { gl::CompileShader(id) };

        // Check compile result
        'b: {
            let mut success = 0;
            unsafe { gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success) };
            
            if success == 1 { break 'b }

            let mut len = 0;
            unsafe { gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len) };

            let mut vec : Vec<u8> = Vec::with_capacity(len as usize);
            unsafe { gl::GetShaderInfoLog(id, len, null_mut(), vec.as_mut_ptr().cast()) }; 

            unsafe { vec.set_len(len as usize - 1) };

            let str = String::from_utf8(vec).unwrap();
            return Err(ShaderError::CompileError(str));
        }

        Ok(Shader { id })
    }
}


impl ShaderProgram {
    pub fn new(fragment: Shader, vertex: Shader) -> Result<ShaderProgram, ShaderProgramError> {
        let program = unsafe { gl::CreateProgram() };
        assert_ne!(program, 0);

        unsafe { gl::AttachShader(program, fragment.id) };
        unsafe { gl::AttachShader(program, vertex.id) };

        unsafe { gl::LinkProgram(program) };

        // Check compile result
        'b: {
            let mut success = 0;
            unsafe { gl::GetProgramiv(program, gl::LINK_STATUS, &mut success) };
            
            if success == 1 { break 'b }

            let mut len = 0;
            unsafe { gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len) };

            let mut vec : Vec<u8> = Vec::with_capacity(len as usize);
            unsafe { gl::GetProgramInfoLog(program, len, null_mut(), vec.as_mut_ptr().cast()) }; 
            unsafe { vec.set_len(len as usize - 1) };

            let str = String::from_utf8(vec).unwrap();
            return Err(ShaderProgramError { info: str });
        }

        Ok(ShaderProgram { id: program })
    }


    pub fn use_program(&self) {
        unsafe { gl::UseProgram(self.id) };
    }


    pub fn set_f32(&self, name: &CStr, value: f32) {
        unsafe {
            let loc = gl::GetUniformLocation(self.id, name.as_ptr());
            gl::Uniform1f(loc, value);
        }
    }



    pub fn set_vec3(&self, name: &CStr, value: Vec3) {
        unsafe {
            let loc = gl::GetUniformLocation(self.id, name.as_ptr());
            gl::Uniform3f(loc, value.x, value.y, value.z);
        }
    }


    pub fn set_vec4(&self, name: &CStr, value: Vec4) {
        unsafe {
            let loc = gl::GetUniformLocation(self.id, name.as_ptr());
            gl::Uniform4f(loc, value.x, value.y, value.z, value.w);
        }
    }


    pub fn set_matrix4(&self, name: &CStr, value: Mat4) {
        unsafe {
            let loc = gl::GetUniformLocation(self.id, name.as_ptr());
            gl::UniformMatrix4fv(loc, 1, gl::FALSE, (&value as *const Mat4).cast());
        }
    }
}


impl ShaderProgramError {
    pub fn read(&self) -> &str { &self.info }
}


impl Drop for Shader {
    fn drop(&mut self) {
        unsafe { gl::DeleteShader(self.id) };
    }
}

