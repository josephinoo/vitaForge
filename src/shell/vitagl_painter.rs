use anyhow::Result;
#[cfg(target_os = "vita")]
use std::collections::HashMap;

#[derive(Default, Clone, Copy)]
pub struct VitaGlPaintStats {
    pub texture_apply_secs: f64,
    pub geometry_secs: f64,
    pub draw_calls: u32,
    pub textures_uploaded: u32,
    pub vertices_drawn: u32,
}

#[cfg(not(target_os = "vita"))]
#[derive(Default)]
pub struct VitaGlEguiPainter;

#[cfg(not(target_os = "vita"))]
impl VitaGlEguiPainter {
    pub fn paint(
        &mut self,
        _screen_size: [u32; 2],
        _pixels_per_point: f32,
        _primitives: &[egui::ClippedPrimitive],
        _textures_delta: &egui::TexturesDelta,
    ) -> Result<VitaGlPaintStats> {
        Ok(VitaGlPaintStats::default())
    }
}

#[cfg(target_os = "vita")]
mod ffi {
    use std::os::raw::{c_char, c_float, c_int, c_uchar, c_uint, c_void};

    pub type GLenum = c_uint;
    pub type GLboolean = c_uchar;
    pub type GLbitfield = c_uint;
    pub type GLint = c_int;
    pub type GLsizei = c_int;
    pub type GLuint = c_uint;
    pub type GLfloat = c_float;

    pub const GL_TRIANGLES: GLenum = 0x0004;
    pub const GL_BLEND: GLenum = 0x0BE2;
    pub const GL_SRC_ALPHA: GLenum = 0x0302;
    pub const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
    pub const GL_SCISSOR_TEST: GLenum = 0x0C11;
    pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
    pub const GL_UNSIGNED_BYTE: GLenum = 0x1401;
    pub const GL_UNSIGNED_SHORT: GLenum = 0x1403;
    pub const GL_FLOAT: GLenum = 0x1406;
    pub const GL_RGBA: GLenum = 0x1908;
    pub const GL_LINEAR: GLenum = 0x2601;
    pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
    pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
    pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
    pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
    pub const GL_CLAMP_TO_EDGE: GLenum = 0x812F;
    pub const GL_ARRAY_BUFFER: GLenum = 0x8892;
    pub const GL_ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
    pub const GL_STREAM_DRAW: GLenum = 0x88E0;
    pub const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
    pub const GL_VERTEX_SHADER: GLenum = 0x8B31;

    unsafe extern "C" {
        pub fn glGenTextures(n: GLsizei, textures: *mut GLuint);
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
            pixels: *const c_void,
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
            pixels: *const c_void,
        );
        pub fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint);
        pub fn glDeleteTextures(n: GLsizei, textures: *const GLuint);

        pub fn glEnable(cap: GLenum);
        pub fn glDisable(cap: GLenum);
        pub fn glBlendFunc(sfactor: GLenum, dfactor: GLenum);
        pub fn glScissor(x: GLint, y: GLint, width: GLsizei, height: GLsizei);
        pub fn glViewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei);

        pub fn glGenBuffers(n: GLsizei, buffers: *mut GLuint);
        pub fn glBindBuffer(target: GLenum, buffer: GLuint);
        pub fn glBufferData(target: GLenum, size: isize, data: *const c_void, usage: GLenum);

        pub fn glCreateShader(type_: GLenum) -> GLuint;
        pub fn glShaderSource(
            shader: GLuint,
            count: GLsizei,
            string: *const *const c_char,
            length: *const GLint,
        );
        pub fn glCompileShader(shader: GLuint);
        pub fn glCreateProgram() -> GLuint;
        pub fn glAttachShader(program: GLuint, shader: GLuint);
        pub fn glLinkProgram(program: GLuint);
        pub fn glUseProgram(program: GLuint);

        pub fn glGetAttribLocation(program: GLuint, name: *const c_char) -> GLint;
        pub fn glGetUniformLocation(program: GLuint, name: *const c_char) -> GLint;
        pub fn glEnableVertexAttribArray(index: GLuint);
        pub fn glVertexAttribPointer(
            index: GLuint,
            size: GLint,
            type_: GLenum,
            normalized: GLboolean,
            stride: GLsizei,
            pointer: *const c_void,
        );
        pub fn glUniform1i(location: GLint, v0: GLint);
        pub fn glUniformMatrix4fv(
            location: GLint,
            count: GLsizei,
            transpose: GLboolean,
            value: *const GLfloat,
        );
        pub fn glDrawElements(
            mode: GLenum,
            count: GLsizei,
            type_: GLenum,
            indices: *const c_void,
        );
    }
}

#[cfg(target_os = "vita")]
pub struct VitaGlEguiPainter {
    textures: HashMap<egui::TextureId, ffi::GLuint>,
    program: ffi::GLuint,
    vbo: ffi::GLuint,
    ibo: ffi::GLuint,
    a_pos: ffi::GLuint,
    a_tc: ffi::GLuint,
    a_srgba: ffi::GLuint,
    u_matrix: ffi::GLint,
    u_sampler: ffi::GLint,
    initialized: bool,
}

#[cfg(target_os = "vita")]
impl Default for VitaGlEguiPainter {
    fn default() -> Self {
        Self {
            textures: HashMap::new(),
            program: 0,
            vbo: 0,
            ibo: 0,
            a_pos: 0,
            a_tc: 0,
            a_srgba: 0,
            u_matrix: -1,
            u_sampler: -1,
            initialized: false,
        }
    }
}

#[cfg(target_os = "vita")]
impl VitaGlEguiPainter {
    fn init(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        unsafe {
            let vs_src = std::ffi::CString::new(
                "attribute vec2 a_pos;\n\
                 attribute vec2 a_tc;\n\
                 attribute vec4 a_srgba;\n\
                 uniform mat4 u_matrix;\n\
                 varying vec2 v_tc;\n\
                 varying vec4 v_srgba;\n\
                 void main() {\n\
                     v_tc = a_tc;\n\
                     v_srgba = a_srgba;\n\
                     gl_Position = u_matrix * vec4(a_pos, 0.0, 1.0);\n\
                 }\n",
            )?;

            let fs_src = std::ffi::CString::new(
                "precision mediump float;\n\
                 uniform sampler2D u_sampler;\n\
                 varying vec2 v_tc;\n\
                 varying vec4 v_srgba;\n\
                 void main() {\n\
                     gl_FragColor = v_srgba * texture2D(u_sampler, v_tc);\n\
                 }\n",
            )?;

            let vs = ffi::glCreateShader(ffi::GL_VERTEX_SHADER);
            let vs_ptr = vs_src.as_ptr();
            ffi::glShaderSource(vs, 1, &vs_ptr, std::ptr::null());
            ffi::glCompileShader(vs);

            let fs = ffi::glCreateShader(ffi::GL_FRAGMENT_SHADER);
            let fs_ptr = fs_src.as_ptr();
            ffi::glShaderSource(fs, 1, &fs_ptr, std::ptr::null());
            ffi::glCompileShader(fs);

            let prog = ffi::glCreateProgram();
            ffi::glAttachShader(prog, vs);
            ffi::glAttachShader(prog, fs);
            ffi::glLinkProgram(prog);
            self.program = prog;

            let pos_str = std::ffi::CString::new("a_pos")?;
            let tc_str = std::ffi::CString::new("a_tc")?;
            let srgba_str = std::ffi::CString::new("a_srgba")?;
            let mat_str = std::ffi::CString::new("u_matrix")?;
            let samp_str = std::ffi::CString::new("u_sampler")?;

            self.a_pos = ffi::glGetAttribLocation(prog, pos_str.as_ptr()) as ffi::GLuint;
            self.a_tc = ffi::glGetAttribLocation(prog, tc_str.as_ptr()) as ffi::GLuint;
            self.a_srgba = ffi::glGetAttribLocation(prog, srgba_str.as_ptr()) as ffi::GLuint;
            self.u_matrix = ffi::glGetUniformLocation(prog, mat_str.as_ptr());
            self.u_sampler = ffi::glGetUniformLocation(prog, samp_str.as_ptr());

            let mut buffers = [0u32; 2];
            ffi::glGenBuffers(2, buffers.as_mut_ptr());
            self.vbo = buffers[0];
            self.ibo = buffers[1];
        }

        self.initialized = true;
        Ok(())
    }

    pub fn paint(
        &mut self,
        screen_size: [u32; 2],
        pixels_per_point: f32,
        primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) -> Result<VitaGlPaintStats> {
        self.init()?;

        let texture_apply_started_at = std::time::Instant::now();
        let mut textures_uploaded = 0u32;

        unsafe {
            for (id, delta) in &textures_delta.set {
                let gl_tex = *self.textures.entry(*id).or_insert_with(|| {
                    let mut tex = 0u32;
                    ffi::glGenTextures(1, &mut tex);
                    ffi::glBindTexture(ffi::GL_TEXTURE_2D, tex);
                    ffi::glTexParameteri(
                        ffi::GL_TEXTURE_2D,
                        ffi::GL_TEXTURE_MAG_FILTER,
                        ffi::GL_LINEAR as i32,
                    );
                    ffi::glTexParameteri(
                        ffi::GL_TEXTURE_2D,
                        ffi::GL_TEXTURE_MIN_FILTER,
                        ffi::GL_LINEAR as i32,
                    );
                    ffi::glTexParameteri(
                        ffi::GL_TEXTURE_2D,
                        ffi::GL_TEXTURE_WRAP_S,
                        ffi::GL_CLAMP_TO_EDGE as i32,
                    );
                    ffi::glTexParameteri(
                        ffi::GL_TEXTURE_2D,
                        ffi::GL_TEXTURE_WRAP_T,
                        ffi::GL_CLAMP_TO_EDGE as i32,
                    );
                    tex
                });

                ffi::glBindTexture(ffi::GL_TEXTURE_2D, gl_tex);
                let pixels: Vec<u8> = match &delta.image {
                    egui::ImageData::Color(image) => image
                        .pixels
                        .iter()
                        .flat_map(|color| color.to_array())
                        .collect(),
                    egui::ImageData::Font(image) => image
                        .srgba_pixels(None)
                        .flat_map(|color| color.to_array())
                        .collect(),
                };

                let [width, height] = delta.image.size();
                if let Some(pos) = delta.pos {
                    ffi::glTexSubImage2D(
                        ffi::GL_TEXTURE_2D,
                        0,
                        pos[0] as i32,
                        pos[1] as i32,
                        width as i32,
                        height as i32,
                        ffi::GL_RGBA,
                        ffi::GL_UNSIGNED_BYTE,
                        pixels.as_ptr() as *const _,
                    );
                } else {
                    ffi::glTexImage2D(
                        ffi::GL_TEXTURE_2D,
                        0,
                        ffi::GL_RGBA as i32,
                        width as i32,
                        height as i32,
                        0,
                        ffi::GL_RGBA,
                        ffi::GL_UNSIGNED_BYTE,
                        pixels.as_ptr() as *const _,
                    );
                }
                textures_uploaded += 1;
            }

            for id in &textures_delta.free {
                if let Some(tex) = self.textures.remove(id) {
                    ffi::glDeleteTextures(1, &tex);
                }
            }
        }
        let texture_apply_secs = texture_apply_started_at.elapsed().as_secs_f64();

        let geometry_started_at = std::time::Instant::now();
        let mut draw_calls = 0u32;
        let mut vertices_drawn = 0u32;

        unsafe {
            ffi::glViewport(0, 0, screen_size[0] as i32, screen_size[1] as i32);
            ffi::glEnable(ffi::GL_BLEND);
            ffi::glBlendFunc(ffi::GL_SRC_ALPHA, ffi::GL_ONE_MINUS_SRC_ALPHA);
            ffi::glEnable(ffi::GL_SCISSOR_TEST);
            ffi::glUseProgram(self.program);

            let width = screen_size[0] as f32 / pixels_per_point;
            let height = screen_size[1] as f32 / pixels_per_point;
            let ortho = [
                2.0 / width, 0.0, 0.0, 0.0,
                0.0, -2.0 / height, 0.0, 0.0,
                0.0, 0.0, -1.0, 0.0,
                -1.0, 1.0, 0.0, 1.0,
            ];
            ffi::glUniformMatrix4fv(self.u_matrix, 1, 0, ortho.as_ptr());
            ffi::glUniform1i(self.u_sampler, 0);

            for clipped_primitive in primitives {
                let clip_rect = clipped_primitive.clip_rect;
                let clip_x = (clip_rect.min.x * pixels_per_point) as i32;
                let clip_y = (screen_size[1] as f32 - (clip_rect.max.y * pixels_per_point)) as i32;
                let clip_w = ((clip_rect.max.x - clip_rect.min.x) * pixels_per_point) as i32;
                let clip_h = ((clip_rect.max.y - clip_rect.min.y) * pixels_per_point) as i32;

                if clip_w <= 0 || clip_h <= 0 {
                    continue;
                }
                ffi::glScissor(clip_x, clip_y, clip_w, clip_h);

                let egui::epaint::Primitive::Mesh(mesh) = &clipped_primitive.primitive else {
                    continue;
                };
                if mesh.vertices.is_empty() || mesh.indices.is_empty() {
                    continue;
                }

                let gl_tex = self.textures.get(&mesh.texture_id).copied().unwrap_or(0);
                ffi::glBindTexture(ffi::GL_TEXTURE_2D, gl_tex);

                let vertices: Vec<f32> = mesh
                    .vertices
                    .iter()
                    .flat_map(|v| {
                        let c = v.color.to_array();
                        [
                            v.pos.x,
                            v.pos.y,
                            v.uv.x,
                            v.uv.y,
                            c[0] as f32 / 255.0,
                            c[1] as f32 / 255.0,
                            c[2] as f32 / 255.0,
                            c[3] as f32 / 255.0,
                        ]
                    })
                    .collect();

                let indices: Vec<u16> = mesh.indices.iter().map(|&i| i as u16).collect();

                ffi::glBindBuffer(ffi::GL_ARRAY_BUFFER, self.vbo);
                ffi::glBufferData(
                    ffi::GL_ARRAY_BUFFER,
                    (vertices.len() * std::mem::size_of::<f32>()) as isize,
                    vertices.as_ptr() as *const _,
                    ffi::GL_STREAM_DRAW,
                );

                ffi::glBindBuffer(ffi::GL_ELEMENT_ARRAY_BUFFER, self.ibo);
                ffi::glBufferData(
                    ffi::GL_ELEMENT_ARRAY_BUFFER,
                    (indices.len() * std::mem::size_of::<u16>()) as isize,
                    indices.as_ptr() as *const _,
                    ffi::GL_STREAM_DRAW,
                );

                let stride = (8 * std::mem::size_of::<f32>()) as i32;
                ffi::glEnableVertexAttribArray(self.a_pos);
                ffi::glVertexAttribPointer(
                    self.a_pos,
                    2,
                    ffi::GL_FLOAT,
                    0,
                    stride,
                    std::ptr::null(),
                );

                ffi::glEnableVertexAttribArray(self.a_tc);
                ffi::glVertexAttribPointer(
                    self.a_tc,
                    2,
                    ffi::GL_FLOAT,
                    0,
                    stride,
                    (2 * std::mem::size_of::<f32>()) as *const _,
                );

                ffi::glEnableVertexAttribArray(self.a_srgba);
                ffi::glVertexAttribPointer(
                    self.a_srgba,
                    4,
                    ffi::GL_FLOAT,
                    0,
                    stride,
                    (4 * std::mem::size_of::<f32>()) as *const _,
                );

                ffi::glDrawElements(
                    ffi::GL_TRIANGLES,
                    indices.len() as i32,
                    ffi::GL_UNSIGNED_SHORT,
                    std::ptr::null(),
                );

                draw_calls += 1;
                vertices_drawn += mesh.vertices.len() as u32;
            }

            ffi::glDisable(ffi::GL_SCISSOR_TEST);
            ffi::glDisable(ffi::GL_BLEND);
        }

        let geometry_secs = geometry_started_at.elapsed().as_secs_f64();
        Ok(VitaGlPaintStats {
            texture_apply_secs,
            geometry_secs,
            draw_calls,
            textures_uploaded,
            vertices_drawn,
        })
    }
}
