#![allow(non_snake_case, dead_code)]

use std::ffi::c_void;

pub type GLboolean = u8;
pub type GLbyte = i8;
pub type GLclampf = f32;
pub type GLdouble = f64;
pub type GLeglImageOES = *mut c_void;
pub type GLfixed = i32;
pub type GLfloat = f32;
pub type GLint = i32;
pub type GLint64 = i64;
pub type GLintptr = isize;
pub type GLshort = i16;
pub type GLsizeiptr = isize;
pub type GLsizei = i32;
pub type GLubyte = u8;
pub type GLuint = u32;
pub type GLuint64 = u64;
pub type GLushort = u16;
pub type GLvoid = c_void;
pub type GLenum = u32;

pub const GL_FALSE: GLboolean = 0;
pub const GL_TRUE: GLboolean = 1;

pub const GL_BYTE: GLenum = 0x1400;
pub const GL_UNSIGNED_BYTE: GLenum = 0x1401;
pub const GL_SHORT: GLenum = 0x1402;
pub const GL_UNSIGNED_SHORT: GLenum = 0x1403;
pub const GL_INT: GLenum = 0x1404;
pub const GL_UNSIGNED_INT: GLenum = 0x1405;
pub const GL_FLOAT: GLenum = 0x1406;

pub const GL_DEPTH_BUFFER_BIT: GLenum = 0x0000_0100;
pub const GL_STENCIL_BUFFER_BIT: GLenum = 0x0000_0400;
pub const GL_COLOR_BUFFER_BIT: GLenum = 0x0000_4000;

pub const GL_TRIANGLES: GLenum = 0x0004;

pub const GL_BLEND: GLenum = 0x0BE2;
pub const GL_SCISSOR_TEST: GLenum = 0x0C11;
pub const GL_DEPTH_TEST: GLenum = 0x0B71;

pub const GL_SRC_ALPHA: GLenum = 0x0302;
pub const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
pub const GL_ONE: GLenum = 1;

pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
pub const GL_TEXTURE0: GLenum = 0x84C0;

pub const GL_RGBA: GLenum = 0x1908;
pub const GL_RGB: GLenum = 0x1907;

pub const GL_LINEAR: GLenum = 0x2601;
pub const GL_NEAREST: GLenum = 0x2600;
pub const GL_CLAMP_TO_EDGE: GLenum = 0x812F;

pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;

pub const GL_ARRAY_BUFFER: GLenum = 0x8892;
pub const GL_ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
pub const GL_STATIC_DRAW: GLenum = 0x88E4;
pub const GL_DYNAMIC_DRAW: GLenum = 0x88E8;

pub const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
pub const GL_VERTEX_SHADER: GLenum = 0x8B31;
pub const GL_COMPILE_STATUS: GLenum = 0x8B81;
pub const GL_LINK_STATUS: GLenum = 0x8B82;
pub const GL_INFO_LOG_LENGTH: GLenum = 0x8B84;

pub const GL_UNPACK_ALIGNMENT: GLenum = 0x0CF5;

pub const GL_NO_ERROR: GLenum = 0;

pub type GLbitfield = u32;
pub type GLchar = i8;

pub const SCE_GXM_MULTISAMPLE_NONE: i32 = 0;

unsafe extern "C" {
    pub fn vglSetCircularPoolSize(size: u32);
    pub fn vglInit(legacy_pool_size: i32) -> GLboolean;
    pub fn vglInitExtended(
        legacy_pool_size: i32,
        width: i32,
        height: i32,
        ram_threshold: i32,
        msaa: i32,
    ) -> GLboolean;
    pub fn vglSwapBuffers(has_commondialog: GLboolean);

    pub fn glClear(mask: GLbitfield);
    pub fn glClearColor(red: GLclampf, green: GLclampf, blue: GLclampf, alpha: GLclampf);
    pub fn glEnable(cap: GLenum);
    pub fn glDisable(cap: GLenum);
    pub fn glBlendFunc(sfactor: GLenum, dfactor: GLenum);
    pub fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
    pub fn glScissor(x: GLint, y: GLint, width: GLsizei, height: GLsizei);

    pub fn glGenTextures(n: GLsizei, textures: *mut GLuint);
    pub fn glDeleteTextures(n: GLsizei, textures: *const GLuint);
    pub fn glBindTexture(target: GLenum, texture: GLuint);
    pub fn glTexImage2D(
        target: GLenum,
        level: GLint,
        internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        type_: GLenum,
        pixels: *const GLvoid,
    );
    pub fn glTexSubImage2D(
        target: GLenum,
        level: GLint,
        xoffset: GLint,
        yoffset: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        type_: GLenum,
        pixels: *const GLvoid,
    );
    pub fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint);

    pub fn glCreateShader(type_: GLenum) -> GLuint;
    pub fn vglShaderGxpBinary(count: GLsizei, handles: *const GLuint, binary: *const GLvoid, length: GLsizei);
    pub fn glUniformMatrix4fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat);
    pub fn glShaderSource(shader: GLuint, count: GLsizei, string: *const *const GLchar, length: *const GLint);
    pub fn glCompileShader(shader: GLuint);
    pub fn glDeleteShader(shader: GLuint);
    pub fn glCreateProgram() -> GLuint;
    pub fn glAttachShader(program: GLuint, shader: GLuint);
    pub fn glLinkProgram(program: GLuint);
    pub fn glDeleteProgram(program: GLuint);
    pub fn glUseProgram(program: GLuint);
    pub fn glGetUniformLocation(program: GLuint, name: *const GLchar) -> GLint;
    pub fn glGetAttribLocation(program: GLuint, name: *const GLchar) -> GLint;
    pub fn glUniform1i(location: GLint, v0: GLint);
    pub fn glUniform1f(location: GLint, v0: GLfloat);
    pub fn glUniform2f(location: GLint, v0: GLfloat, v1: GLfloat);

    pub fn glGenBuffers(n: GLsizei, buffers: *mut GLuint);
    pub fn glDeleteBuffers(n: GLsizei, buffers: *const GLuint);
    pub fn glBindBuffer(target: GLenum, buffer: GLuint);
    pub fn glBufferData(target: GLenum, size: GLsizeiptr, data: *const GLvoid, usage: GLenum);
    pub fn glBufferSubData(target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *const GLvoid);

    pub fn glDrawElements(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const GLvoid);
    pub fn glVertexAttribPointer(
        index: GLuint,
        size: GLint,
        type_: GLenum,
        normalized: GLboolean,
        stride: GLsizei,
        pointer: *const GLvoid,
    );
    pub fn glEnableVertexAttribArray(index: GLuint);
    pub fn glDisableVertexAttribArray(index: GLuint);

    pub fn glActiveTexture(texture: GLenum);
    pub fn glPixelStorei(pname: GLenum, param: GLint);

    pub fn glGetError() -> GLenum;
    pub fn glGetShaderiv(shader: GLuint, pname: GLenum, params: *mut GLint);
    pub fn glGetShaderInfoLog(shader: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar);
    pub fn glGetProgramiv(program: GLuint, pname: GLenum, params: *mut GLint);
    pub fn glGetProgramInfoLog(program: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar);
}
