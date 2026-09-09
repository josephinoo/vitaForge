use anyhow::{Result, bail};
use std::collections::{HashMap, VecDeque};
use std::ptr;

use crate::app::icons::{HERO_SIDE, MAX_ICON_SIDE};
use crate::shell::vgl::*;

const MAX_UPLOAD_ATTEMPTS: u32 = 8;
const BACKOFF_RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(150);
const NEW_TEXTURES_PER_FRAME: usize = 2;
const MAX_UPLOAD_BYTES_PER_FRAME: usize = 128 * 1024;
const MAX_PENDING_SERVICED_PER_FRAME: usize = 2;
pub const INITIAL_ICON_POOL_SIZE: usize = 24;
pub const ICON_FREE_POOL_CAP: usize = 32;
pub const ICON_FREE_POOL_WARM_LOW: usize = 8;
const HERO_FREE_POOL_CAP: usize = 6;
const MAX_PENDING_UPLOADS: usize = 16;
const VERTEX_STRIDE: usize = 8;
const INITIAL_VBO_VERTS: usize = 4096;
const INITIAL_IBO_INDICES: usize = 8192;

#[repr(align(16))]
struct AlignedShader<const N: usize>([u8; N]);
static VERTEX_SHADER: AlignedShader<400> =
    AlignedShader(*include_bytes!("../../assets/shaders/egui/vertex.gxp"));
static FRAGMENT_SHADER: AlignedShader<288> =
    AlignedShader(*include_bytes!("../../assets/shaders/egui/fragment.gxp"));

struct FrameUploadBudget {
    creates: usize,
    bytes: usize,
}
impl FrameUploadBudget {
    fn fresh() -> Self {
        Self {
            creates: 0,
            bytes: 0,
        }
    }
    fn can_create(&self) -> bool {
        self.creates < NEW_TEXTURES_PER_FRAME
    }
    fn can_upload_bytes(&self, n: usize) -> bool {
        self.bytes == 0 || self.bytes.saturating_add(n) <= MAX_UPLOAD_BYTES_PER_FRAME
    }
    fn record_create(&mut self) {
        self.creates += 1;
    }
    fn record_bytes(&mut self, n: usize) {
        self.bytes = self.bytes.saturating_add(n);
    }
    fn busy(&self) -> bool {
        self.creates > 0 || self.bytes > 0
    }
}

pub struct GlEguiPainter {
    textures: HashMap<egui::TextureId, GlEguiTexture>,
    pending: HashMap<egui::TextureId, PendingUpload>,
    pending_order: VecDeque<egui::TextureId>,
    dropped: Vec<egui::TextureId>,
    icon_free_pool: Vec<GLuint>,
    hero_free_pool: Vec<GLuint>,
    vertices: Vec<f32>,
    indices: Vec<u32>,
    scratch: Vec<u8>,
    program: GLuint,
    vbo: GLuint,
    ibo: GLuint,
    vbo_bytes: usize,
    ibo_bytes: usize,
    attr_pos: GLint,
    attr_uv: GLint,
    attr_color: GLint,
    uniform_projection: GLint,
    white_texture: GLuint,
    frame_alpha: f32,
    screen_w: u32,
    screen_h: u32,
    initialized: bool,
}

struct GlEguiTexture {
    texture: GLuint,
    uv_scale: egui::Vec2,
    width: u32,
    height: u32,
}

struct PendingUpload {
    size: [usize; 2],
    pos: Option<[usize; 2]>,
    pixels: Vec<u8>,
    attempts: u32,
    next_retry_at: std::time::Instant,
}

#[derive(Default, Clone, Copy)]
pub struct PaintStats {
    pub texture_apply_secs: f64,
    pub geometry_secs: f64,
    pub draw_calls: u32,
    pub textures_uploaded: u32,
    pub vertices_drawn: u32,
    pub missing_textures: u32,
}

impl Default for GlEguiPainter {
    fn default() -> Self {
        Self::new()
    }
}

impl GlEguiPainter {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            pending: HashMap::new(),
            pending_order: VecDeque::new(),
            dropped: Vec::new(),
            icon_free_pool: Vec::new(),
            hero_free_pool: Vec::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            scratch: Vec::new(),
            program: 0,
            vbo: 0,
            ibo: 0,
            vbo_bytes: 0,
            ibo_bytes: 0,
            attr_pos: -1,
            attr_uv: -1,
            attr_color: -1,
            uniform_projection: -1,
            white_texture: 0,
            frame_alpha: 1.0,
            screen_w: 0,
            screen_h: 0,
            initialized: false,
        }
    }

    pub fn init(&mut self, screen_w: u32, screen_h: u32) -> Result<()> {
        self.screen_w = screen_w;
        self.screen_h = screen_h;
        unsafe {
            glEnable(GL_BLEND);
            glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
            glDisable(GL_DEPTH_TEST);
            glActiveTexture(GL_TEXTURE0);

            self.program = link_program(&VERTEX_SHADER.0, &FRAGMENT_SHADER.0)?;
            self.attr_pos =
                glGetAttribLocation(self.program, c"aPosition".as_ptr() as *const GLchar);
            self.attr_uv = glGetAttribLocation(self.program, c"aTexcoord".as_ptr() as *const GLchar);
            self.attr_color =
                glGetAttribLocation(self.program, c"aColor".as_ptr() as *const GLchar);
            self.uniform_projection = glGetUniformLocation(
                self.program,
                c"wvp".as_ptr() as *const GLchar,
            );
            if self.attr_pos < 0 || self.attr_uv < 0 || self.attr_color < 0
                || self.uniform_projection < 0 {
                bail!("GXP shader attributes or projection uniform missing");
            }
            let error = glGetError();
            if error != GL_NO_ERROR {
                bail!("GXP shader initialization failed with GL error 0x{error:X}");
            }

            glGenBuffers(1, &mut self.vbo);
            glGenBuffers(1, &mut self.ibo);
            self.ensure_buffer_capacity(INITIAL_VBO_VERTS, INITIAL_IBO_INDICES);

            self.white_texture = create_empty_texture(1, 1)?;
            let white = [255u8, 255, 255, 255];
            glBindTexture(GL_TEXTURE_2D, self.white_texture);
            glTexSubImage2D(
                GL_TEXTURE_2D,
                0,
                0,
                0,
                1,
                1,
                GL_RGBA,
                GL_UNSIGNED_BYTE,
                white.as_ptr() as *const _,
            );
        }
        self.initialized = true;
        Ok(())
    }

    pub fn prewarm(&mut self) {
        for _ in 0..INITIAL_ICON_POOL_SIZE {
            if let Ok(texture) = create_empty_texture(MAX_ICON_SIDE, MAX_ICON_SIDE) {
                self.icon_free_pool.push(texture);
            }
        }
        for _ in 0..2 {
            if let Ok(texture) = create_empty_texture(HERO_SIDE, HERO_SIDE) {
                self.hero_free_pool.push(texture);
            }
        }
    }

    pub fn pending_uploads(&self) -> usize {
        self.pending.len()
    }

    pub fn take_dropped_textures(&mut self) -> Vec<egui::TextureId> {
        std::mem::take(&mut self.dropped)
    }

    pub fn set_frame_alpha(&mut self, alpha: f32) {
        self.frame_alpha = alpha.clamp(0.0, 1.0);
    }

    pub fn clear(&self) {
        if !self.initialized {
            return;
        }
        unsafe {
            glClearColor(0.0, 0.0, 0.0, 1.0);
            glClear(GL_COLOR_BUFFER_BIT);
        }
    }

    pub fn paint(
        &mut self,
        screen_size: [u32; 2],
        pixels_per_point: f32,
        primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) -> Result<PaintStats> {
        let texture_apply_started_at = std::time::Instant::now();
        let textures_uploaded = self.apply_textures(textures_delta);
        let texture_apply_secs = texture_apply_started_at.elapsed().as_secs_f64();

        let geometry_started_at = std::time::Instant::now();
        unsafe {
            glViewport(0, 0, screen_size[0] as i32, screen_size[1] as i32);
            glUseProgram(self.program);
            let projection = [
                2.0 / screen_size[0] as f32, 0.0, 0.0, 0.0,
                0.0, -2.0 / screen_size[1] as f32, 0.0, 0.0,
                0.0, 0.0, -2.0, 0.0,
                -1.0, 1.0, 1.0, 1.0,
            ];
            glUniformMatrix4fv(self.uniform_projection, 1, GL_FALSE, projection.as_ptr());
            glBindBuffer(GL_ARRAY_BUFFER, self.vbo);
            glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, self.ibo);
            let stride = (VERTEX_STRIDE * std::mem::size_of::<f32>()) as i32;
            glVertexAttribPointer(
                self.attr_pos as u32,
                2,
                GL_FLOAT,
                GL_FALSE,
                stride,
                ptr::null(),
            );
            glEnableVertexAttribArray(self.attr_pos as u32);
            glVertexAttribPointer(
                self.attr_uv as u32,
                2,
                GL_FLOAT,
                GL_FALSE,
                stride,
                (2 * std::mem::size_of::<f32>()) as *const _,
            );
            glEnableVertexAttribArray(self.attr_uv as u32);
            glVertexAttribPointer(
                self.attr_color as u32,
                4,
                GL_FLOAT,
                GL_FALSE,
                stride,
                (4 * std::mem::size_of::<f32>()) as *const _,
            );
            glEnableVertexAttribArray(self.attr_color as u32);
            glEnable(GL_SCISSOR_TEST);
        }

        let mut draw_calls = 0u32;
        let mut vertices_drawn = 0u32;
        let mut missing_textures = 0u32;
        let mut current_clip: Option<[i32; 4]> = None;
        let mut current_texture_id: Option<egui::TextureId> = None;
        struct DrawRange {
            texture_id: Option<egui::TextureId>,
            clip: [i32; 4],
            index_start: usize,
            index_count: i32,
            vertex_count: u32,
        }
        let mut draw_ranges: Vec<DrawRange> = Vec::with_capacity(64);
        self.vertices.clear();
        self.indices.clear();
        let mut range_index_start = 0usize;
        let mut range_vert_start = 0usize;

        for clipped_primitive in primitives {
            let Some(clip_rect) =
                Self::gl_clip_rect(clipped_primitive.clip_rect, screen_size, pixels_per_point)
            else {
                continue;
            };
            let egui::epaint::Primitive::Mesh(mesh) = &clipped_primitive.primitive else {
                continue;
            };
            if mesh.indices.is_empty() || mesh.vertices.is_empty() {
                continue;
            }
            let uv_scale = match self.textures.get(&mesh.texture_id) {
                Some(t) => t.uv_scale,
                None => {
                    missing_textures += 1;
                    continue;
                }
            };
            let same_batch =
                current_clip == Some(clip_rect) && current_texture_id == Some(mesh.texture_id);
            if !same_batch {
                if let Some(clip) = current_clip {
                    let index_count = self.indices.len() - range_index_start;
                    if index_count > 0 {
                        let vertex_count =
                            ((self.vertices.len() / VERTEX_STRIDE) - range_vert_start) as u32;
                        draw_ranges.push(DrawRange {
                            texture_id: current_texture_id,
                            clip,
                            index_start: range_index_start,
                            index_count: index_count as i32,
                            vertex_count,
                        });
                        range_index_start = self.indices.len();
                        range_vert_start = self.vertices.len() / VERTEX_STRIDE;
                    }
                }
                current_clip = Some(clip_rect);
                current_texture_id = Some(mesh.texture_id);
            }
            let base_index = (self.vertices.len() / VERTEX_STRIDE) as u32;
            for vertex in &mesh.vertices {
                self.vertices
                    .extend(Self::gl_vertex(vertex, pixels_per_point, uv_scale, self.frame_alpha));
            }
            self.indices
                .extend(mesh.indices.iter().map(|&i| base_index + i));
        }
        if let Some(clip) = current_clip {
            let index_count = self.indices.len() - range_index_start;
            if index_count > 0 {
                let vertex_count =
                    ((self.vertices.len() / VERTEX_STRIDE) - range_vert_start) as u32;
                draw_ranges.push(DrawRange {
                    texture_id: current_texture_id,
                    clip,
                    index_start: range_index_start,
                    index_count: index_count as i32,
                    vertex_count,
                });
            }
        }

        if !self.indices.is_empty() {
            let vert_count = self.vertices.len() / VERTEX_STRIDE;
            self.ensure_buffer_capacity(vert_count, self.indices.len());
            unsafe {
                glBindBuffer(GL_ARRAY_BUFFER, self.vbo);
                glBufferSubData(
                    GL_ARRAY_BUFFER,
                    0,
                    (self.vertices.len() * std::mem::size_of::<f32>()) as isize,
                    self.vertices.as_ptr() as *const _,
                );
                glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, self.ibo);
                glBufferSubData(
                    GL_ELEMENT_ARRAY_BUFFER,
                    0,
                    (self.indices.len() * std::mem::size_of::<u32>()) as isize,
                    self.indices.as_ptr() as *const _,
                );
            }
            for range in &draw_ranges {
                let gl_texture = range
                    .texture_id
                    .and_then(|id| self.textures.get(&id))
                    .map(|t| t.texture)
                    .unwrap_or(self.white_texture);
                self.apply_scissor(range.clip);
                unsafe {
                    glBindTexture(GL_TEXTURE_2D, gl_texture);
                    glDrawElements(
                        GL_TRIANGLES,
                        range.index_count,
                        GL_UNSIGNED_INT,
                        (range.index_start * std::mem::size_of::<u32>()) as *const _,
                    );
                }
                draw_calls += 1;
                vertices_drawn += range.vertex_count;
            }
        }
        self.vertices.clear();
        self.indices.clear();

        unsafe {
            glDisable(GL_SCISSOR_TEST);
        }

        let geometry_secs = geometry_started_at.elapsed().as_secs_f64();

        for texture_id in &textures_delta.free {
            self.pending.remove(texture_id);
            let Some(freed) = self.textures.remove(texture_id) else {
                continue;
            };
            self.recycle(freed.texture, freed.width, freed.height);
        }

        Ok(PaintStats {
            texture_apply_secs,
            geometry_secs,
            draw_calls,
            textures_uploaded,
            vertices_drawn,
            missing_textures,
        })
    }

    pub fn force_recover(&mut self) -> Vec<egui::TextureId> {
        self.release_pools();
        let stale: Vec<egui::TextureId> = self
            .textures
            .keys()
            .copied()
            .filter(|id| !Self::is_font_atlas(*id))
            .collect();
        for id in &stale {
            if let Some(texture) = self.textures.remove(id) {
                delete_texture(texture.texture);
            }
        }
        self.pending.retain(|id, _| Self::is_font_atlas(*id));
        self.pending_order.retain(|id| Self::is_font_atlas(*id));
        stale
    }

    fn apply_scissor(&self, clip: [i32; 4]) {
        let [x, y, w, h] = clip;
        let fb_h = self.screen_h as i32;
        unsafe {
            glScissor(x, fb_h - y - h, w, h);
        }
    }

    fn ensure_buffer_capacity(&mut self, vert_count: usize, index_count: usize) {
        let vert_bytes = vert_count * VERTEX_STRIDE * std::mem::size_of::<f32>();
        let index_bytes = index_count * std::mem::size_of::<u32>();
        unsafe {
            if vert_bytes > self.vbo_bytes {
                self.vbo_bytes = vert_bytes.next_power_of_two().max(INITIAL_VBO_VERTS * 32);
                glBindBuffer(GL_ARRAY_BUFFER, self.vbo);
                glBufferData(
                    GL_ARRAY_BUFFER,
                    self.vbo_bytes as isize,
                    ptr::null(),
                    GL_DYNAMIC_DRAW,
                );
            }
            if index_bytes > self.ibo_bytes {
                self.ibo_bytes = index_bytes
                    .next_power_of_two()
                    .max(INITIAL_IBO_INDICES * 4);
                glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, self.ibo);
                glBufferData(
                    GL_ELEMENT_ARRAY_BUFFER,
                    self.ibo_bytes as isize,
                    ptr::null(),
                    GL_DYNAMIC_DRAW,
                );
            }
        }
    }

    fn is_new_creation(&self, texture_id: egui::TextureId, pos: Option<[usize; 2]>) -> bool {
        pos.is_none() || !self.textures.contains_key(&texture_id)
    }

    fn is_font_atlas(texture_id: egui::TextureId) -> bool {
        texture_id == egui::TextureId::default()
    }

    fn can_serve(&self, size: [usize; 2], budget: &FrameUploadBudget) -> bool {
        if budget.can_create() {
            return true;
        }
        if Self::is_icon_class_size(size) {
            return !self.icon_free_pool.is_empty();
        }
        if Self::is_hero_class_size(size) {
            return !self.hero_free_pool.is_empty();
        }
        false
    }

    fn is_icon_class_size(size: [usize; 2]) -> bool {
        size[0] as u32 <= MAX_ICON_SIDE && size[1] as u32 <= MAX_ICON_SIDE
    }

    fn is_hero_class_size(size: [usize; 2]) -> bool {
        size[0] as u32 <= HERO_SIDE
            && size[1] as u32 <= HERO_SIDE
            && !Self::is_icon_class_size(size)
    }

    fn apply_textures(&mut self, textures_delta: &egui::TexturesDelta) -> u32 {
        let mut uploaded = 0u32;
        let mut budget = FrameUploadBudget::fresh();
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut serviced_this_frame = 0usize;

        for (texture_id, delta) in &textures_delta.set {
            if !Self::is_font_atlas(*texture_id) {
                continue;
            }
            scratch.clear();
            Self::fill_rgba(&delta.image, &mut scratch);
            self.pending.remove(texture_id);
            self.upload(*texture_id, delta.image.size(), delta.pos, &scratch, 0);
            uploaded += 1;
        }
        if let Some(upload) = self.pending.remove(&egui::TextureId::default()) {
            self.upload(
                egui::TextureId::default(),
                upload.size,
                upload.pos,
                &upload.pixels,
                upload.attempts,
            );
            uploaded += 1;
        }

        let now = std::time::Instant::now();
        let mut not_serviced: Vec<egui::TextureId> = Vec::new();
        for _ in 0..self.pending_order.len() {
            if serviced_this_frame >= MAX_PENDING_SERVICED_PER_FRAME {
                break;
            }
            let Some(texture_id) = self.pending_order.pop_front() else {
                break;
            };
            if Self::is_font_atlas(texture_id) {
                continue;
            }
            let Some(upload) = self.pending.remove(&texture_id) else {
                continue;
            };
            let pixel_bytes = upload.pixels.len();
            if upload.next_retry_at > now
                || !budget.can_upload_bytes(pixel_bytes)
                || !self.can_serve(upload.size, &budget)
            {
                self.pending.insert(texture_id, upload);
                not_serviced.push(texture_id);
                continue;
            }
            serviced_this_frame += 1;
            let outcome = if Self::is_hero_class_size(upload.size) {
                self.try_upload_hero(texture_id, upload.size, &upload.pixels, &mut budget)
            } else if Self::is_icon_class_size(upload.size) {
                self.try_upload_icon(texture_id, upload.size, &upload.pixels, &mut budget)
            } else {
                self.upload(texture_id, upload.size, upload.pos, &upload.pixels, upload.attempts);
                UploadOutcome::Done { created: true }
            };
            if let UploadOutcome::Done { created } = outcome {
                if created {
                    budget.record_create();
                }
                budget.record_bytes(pixel_bytes);
                uploaded += 1;
            }
        }
        for texture_id in not_serviced.into_iter().rev() {
            self.pending_order.push_front(texture_id);
        }

        for (texture_id, delta) in &textures_delta.set {
            if Self::is_font_atlas(*texture_id) {
                continue;
            }
            let is_new = self.is_new_creation(*texture_id, delta.pos);
            let [width, height] = delta.image.size();
            let pixel_bytes = width.saturating_mul(height).saturating_mul(4);
            if is_new
                && (serviced_this_frame >= MAX_PENDING_SERVICED_PER_FRAME
                    || !budget.can_upload_bytes(pixel_bytes))
            {
                self.defer_image(*texture_id, &delta.image, delta.pos, &mut scratch);
                continue;
            }
            scratch.clear();
            Self::fill_rgba(&delta.image, &mut scratch);
            if is_new && Self::is_hero_class_size(delta.image.size()) {
                let would_create = self.hero_free_pool.is_empty();
                if (would_create && !budget.can_create()) || !budget.can_upload_bytes(pixel_bytes) {
                    self.defer_upload(*texture_id, delta.image.size(), None, &scratch);
                    continue;
                }
                serviced_this_frame += 1;
                match self.try_upload_hero(*texture_id, delta.image.size(), &scratch, &mut budget)
                {
                    UploadOutcome::Done { created } => {
                        if created {
                            budget.record_create();
                        }
                        budget.record_bytes(pixel_bytes);
                        uploaded += 1;
                    }
                    UploadOutcome::Deferred => {}
                }
                continue;
            }
            if is_new && Self::is_icon_class_size(delta.image.size()) {
                let would_create = self.icon_free_pool.is_empty();
                if (would_create && !budget.can_create()) || !budget.can_upload_bytes(pixel_bytes) {
                    self.defer_upload(*texture_id, delta.image.size(), None, &scratch);
                    continue;
                }
                serviced_this_frame += 1;
                match self.try_upload_icon(*texture_id, delta.image.size(), &scratch, &mut budget) {
                    UploadOutcome::Done { created } => {
                        if created {
                            budget.record_create();
                        }
                        budget.record_bytes(pixel_bytes);
                        uploaded += 1;
                    }
                    UploadOutcome::Deferred => {}
                }
                continue;
            }
            if is_new && (!budget.can_create() || !budget.can_upload_bytes(pixel_bytes)) {
                self.defer_upload(*texture_id, delta.image.size(), delta.pos, &scratch);
                continue;
            }
            if is_new {
                budget.record_create();
                serviced_this_frame += 1;
            }
            budget.record_bytes(pixel_bytes);
            self.upload(*texture_id, delta.image.size(), delta.pos, &scratch, 0);
            uploaded += 1;
        }
        self.scratch = scratch;
        if !budget.busy()
            && self.pending.is_empty()
            && textures_delta.set.is_empty()
            && self.icon_free_pool.len() < ICON_FREE_POOL_WARM_LOW
            && let Ok(texture) = create_empty_texture(MAX_ICON_SIDE, MAX_ICON_SIDE)
        {
            self.icon_free_pool.push(texture);
        }
        uploaded
    }

    fn try_upload_hero(
        &mut self,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pixels: &[u8],
        budget: &mut FrameUploadBudget,
    ) -> UploadOutcome {
        if let Some(texture) = self.hero_free_pool.pop() {
            self.finish_hero_upload(texture, texture_id, size, pixels);
            return UploadOutcome::Done { created: false };
        }
        if !budget.can_create() {
            self.defer_or_give_up(texture_id, size, None, pixels, 0);
            return UploadOutcome::Deferred;
        }
        match create_empty_texture(HERO_SIDE, HERO_SIDE) {
            Ok(texture) => {
                self.finish_hero_upload(texture, texture_id, size, pixels);
                UploadOutcome::Done { created: true }
            }
            Err(mut err) => {
                if self.release_pools() > 0 {
                    match create_empty_texture(HERO_SIDE, HERO_SIDE) {
                        Ok(texture) => {
                            self.finish_hero_upload(texture, texture_id, size, pixels);
                            return UploadOutcome::Done { created: true };
                        }
                        Err(retry_err) => err = retry_err,
                    }
                }
                eprintln!("no room for a {HERO_SIDE}x{HERO_SIDE} hero texture, will retry: {err}");
                self.defer_or_give_up(texture_id, size, None, pixels, 0);
                UploadOutcome::Deferred
            }
        }
    }

    fn finish_hero_upload(
        &mut self,
        texture: GLuint,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pixels: &[u8],
    ) {
        let [width, height] = size;
        if let Err(err) = upload_texture_region(texture, 0, 0, width as u32, height as u32, pixels, width)
        {
            eprintln!("couldn't patch a pooled hero texture, will retry: {err}");
            if self.hero_free_pool.len() < HERO_FREE_POOL_CAP {
                self.hero_free_pool.push(texture);
            } else {
                delete_texture(texture);
            }
            self.defer_or_give_up(texture_id, size, None, pixels, 0);
            return;
        }
        let cap = HERO_SIDE as f32;
        let uv_scale = egui::vec2(width as f32 / cap, height as f32 / cap);
        if let Some(previous) = self.textures.insert(
            texture_id,
            GlEguiTexture {
                texture,
                uv_scale,
                width: HERO_SIDE,
                height: HERO_SIDE,
            },
        ) {
            self.recycle(previous.texture, previous.width, previous.height);
        }
    }

    fn try_upload_icon(
        &mut self,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pixels: &[u8],
        budget: &mut FrameUploadBudget,
    ) -> UploadOutcome {
        if let Some(texture) = self.icon_free_pool.pop() {
            self.finish_icon_upload(texture, texture_id, size, pixels);
            return UploadOutcome::Done { created: false };
        }
        if !budget.can_create() {
            self.defer_or_give_up(texture_id, size, None, pixels, 0);
            return UploadOutcome::Deferred;
        }
        match create_empty_texture(MAX_ICON_SIDE, MAX_ICON_SIDE) {
            Ok(texture) => {
                self.finish_icon_upload(texture, texture_id, size, pixels);
                UploadOutcome::Done { created: true }
            }
            Err(mut err) => {
                if self.release_pools() > 0 {
                    match create_empty_texture(MAX_ICON_SIDE, MAX_ICON_SIDE) {
                        Ok(texture) => {
                            self.finish_icon_upload(texture, texture_id, size, pixels);
                            return UploadOutcome::Done { created: true };
                        }
                        Err(retry_err) => err = retry_err,
                    }
                }
                eprintln!(
                    "no room for a {MAX_ICON_SIDE}x{MAX_ICON_SIDE} icon texture, will retry: {err}"
                );
                self.defer_or_give_up(texture_id, size, None, pixels, 0);
                UploadOutcome::Deferred
            }
        }
    }

    fn finish_icon_upload(
        &mut self,
        texture: GLuint,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pixels: &[u8],
    ) {
        let [width, height] = size;
        if let Err(err) =
            upload_texture_region(texture, 0, 0, width as u32, height as u32, pixels, width)
        {
            eprintln!("couldn't patch a pooled icon texture, will retry: {err}");
            if self.icon_free_pool.len() < ICON_FREE_POOL_CAP {
                self.icon_free_pool.push(texture);
            } else {
                delete_texture(texture);
            }
            self.defer_or_give_up(texture_id, size, None, pixels, 0);
            return;
        }
        let cap = MAX_ICON_SIDE as f32;
        let uv_scale = egui::vec2(width as f32 / cap, height as f32 / cap);
        if let Some(previous) = self.textures.insert(
            texture_id,
            GlEguiTexture {
                texture,
                uv_scale,
                width: MAX_ICON_SIDE,
                height: MAX_ICON_SIDE,
            },
        ) {
            self.recycle(previous.texture, previous.width, previous.height);
        }
    }

    fn release_pools(&mut self) -> usize {
        let freed = self.icon_free_pool.len() + self.hero_free_pool.len();
        for texture in self.icon_free_pool.drain(..) {
            delete_texture(texture);
        }
        for texture in self.hero_free_pool.drain(..) {
            delete_texture(texture);
        }
        freed
    }

    fn recycle(&mut self, texture: GLuint, width: u32, height: u32) {
        if width == MAX_ICON_SIDE
            && height == MAX_ICON_SIDE
            && self.icon_free_pool.len() < ICON_FREE_POOL_CAP
        {
            self.icon_free_pool.push(texture);
        } else if width == HERO_SIDE
            && height == HERO_SIDE
            && self.hero_free_pool.len() < HERO_FREE_POOL_CAP
        {
            self.hero_free_pool.push(texture);
        } else {
            delete_texture(texture);
        }
    }

    fn make_pending_room(&mut self, keep: egui::TextureId) {
        while self.pending.len() >= MAX_PENDING_UPLOADS {
            let victim = self
                .pending
                .iter()
                .filter(|(id, _)| **id != keep && !Self::is_font_atlas(**id))
                .max_by_key(|(_, upload)| upload.attempts)
                .map(|(id, _)| *id);
            let Some(victim) = victim else { break };
            self.pending.remove(&victim);
            self.dropped.push(victim);
        }
    }

    fn defer_upload(
        &mut self,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pos: Option<[usize; 2]>,
        pixels: &[u8],
    ) {
        self.enqueue_pending(
            texture_id,
            PendingUpload {
                size,
                pos,
                pixels: pixels.to_vec(),
                attempts: 0,
                next_retry_at: std::time::Instant::now(),
            },
        );
    }

    fn defer_image(
        &mut self,
        texture_id: egui::TextureId,
        image: &egui::ImageData,
        pos: Option<[usize; 2]>,
        scratch: &mut Vec<u8>,
    ) {
        scratch.clear();
        Self::fill_rgba(image, scratch);
        self.enqueue_pending(
            texture_id,
            PendingUpload {
                size: image.size(),
                pos,
                pixels: scratch.to_vec(),
                attempts: 0,
                next_retry_at: std::time::Instant::now(),
            },
        );
    }

    fn enqueue_pending(&mut self, texture_id: egui::TextureId, upload: PendingUpload) {
        self.make_pending_room(texture_id);
        if self.pending.insert(texture_id, upload).is_none() {
            self.pending_order.push_back(texture_id);
        }
        if self.pending_order.len() > MAX_PENDING_UPLOADS * 4 {
            let live = std::mem::take(&mut self.pending_order);
            self.pending_order = live
                .into_iter()
                .filter(|id| self.pending.contains_key(id))
                .collect();
        }
    }

    fn defer_or_give_up(
        &mut self,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pos: Option<[usize; 2]>,
        pixels: &[u8],
        attempts: u32,
    ) {
        let attempts = attempts + 1;
        if attempts >= MAX_UPLOAD_ATTEMPTS && !Self::is_font_atlas(texture_id) {
            eprintln!(
                "giving up on a {}x{} texture after {attempts} attempts",
                size[0], size[1]
            );
            self.pending.remove(&texture_id);
            self.dropped.push(texture_id);
            return;
        }
        let attempts = if Self::is_font_atlas(texture_id) {
            attempts.min(MAX_UPLOAD_ATTEMPTS.saturating_sub(1))
        } else {
            attempts
        };
        self.enqueue_pending(
            texture_id,
            PendingUpload {
                size,
                pos,
                pixels: pixels.to_vec(),
                attempts,
                next_retry_at: std::time::Instant::now() + BACKOFF_RETRY_INTERVAL,
            },
        );
    }

    fn upload(
        &mut self,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pos: Option<[usize; 2]>,
        pixels: &[u8],
        attempts: u32,
    ) {
        let [width, height] = size;
        if pos.is_none() || !self.textures.contains_key(&texture_id) {
            let mut created = create_empty_texture(width as u32, height as u32);
            if created.is_err() && self.release_pools() > 0 {
                created = create_empty_texture(width as u32, height as u32);
            }
            let texture = match created {
                Ok(texture) => texture,
                Err(err) => {
                    eprintln!("no room for a {width}x{height} texture, will retry: {err}");
                    self.defer_or_give_up(texture_id, size, pos, pixels, attempts);
                    return;
                }
            };
            if let Err(err) = upload_texture_region(
                texture,
                0,
                0,
                width as u32,
                height as u32,
                pixels,
                width,
            ) {
                eprintln!("couldn't upload a texture, will retry: {err}");
                delete_texture(texture);
                self.defer_or_give_up(texture_id, size, pos, pixels, attempts);
                return;
            }
            if let Some(previous) = self.textures.insert(
                texture_id,
                GlEguiTexture {
                    texture,
                    uv_scale: egui::vec2(1.0, 1.0),
                    width: width as u32,
                    height: height as u32,
                },
            ) {
                delete_texture(previous.texture);
            }
            return;
        }
        let Some([x, y]) = pos else {
            eprintln!("partial texture update with no position, skipped");
            return;
        };
        let Some(existing) = self.textures.get(&texture_id) else {
            eprintln!("partial update for a texture that no longer exists, skipped");
            return;
        };
        if let Err(err) = upload_texture_region(
            existing.texture,
            x as u32,
            y as u32,
            width as u32,
            height as u32,
            pixels,
            width,
        ) {
            eprintln!("couldn't patch a texture: {err}");
        }
    }

    fn fill_rgba(image: &egui::ImageData, out: &mut Vec<u8>) {
        match image {
            egui::ImageData::Color(image) => {
                let bytes: &[u8] = unsafe {
                    std::slice::from_raw_parts(
                        image.pixels.as_ptr() as *const u8,
                        image.pixels.len() * 4,
                    )
                };
                out.extend_from_slice(bytes);
            }
            egui::ImageData::Font(image) => {
                for pixel in image.srgba_pixels(None) {
                    out.extend_from_slice(&pixel.to_srgba_unmultiplied());
                }
            }
        }
    }

    fn gl_vertex(
        vertex: &egui::epaint::Vertex,
        pixels_per_point: f32,
        uv_scale: egui::Vec2,
        frame_alpha: f32,
    ) -> [f32; VERTEX_STRIDE] {
        let [r, g, b, a] = vertex.color.to_array();
        let a_norm = a as f32 / 255.0 * frame_alpha;
        let r_premult = r as f32 / 255.0 * frame_alpha;
        let g_premult = g as f32 / 255.0 * frame_alpha;
        let b_premult = b as f32 / 255.0 * frame_alpha;
        [
            vertex.pos.x * pixels_per_point,
            vertex.pos.y * pixels_per_point,
            (vertex.uv.x * uv_scale.x).clamp(0.0, 1.0),
            (vertex.uv.y * uv_scale.y).clamp(0.0, 1.0),
            r_premult,
            g_premult,
            b_premult,
            a_norm,
        ]
    }

    fn gl_clip_rect(
        clip_rect: egui::Rect,
        [screen_width, screen_height]: [u32; 2],
        pixels_per_point: f32,
    ) -> Option<[i32; 4]> {
        let min_x = (clip_rect.min.x * pixels_per_point)
            .floor()
            .clamp(0.0, screen_width as f32) as i32;
        let min_y = (clip_rect.min.y * pixels_per_point)
            .floor()
            .clamp(0.0, screen_height as f32) as i32;
        let max_x = (clip_rect.max.x * pixels_per_point)
            .ceil()
            .clamp(0.0, screen_width as f32) as i32;
        let max_y = (clip_rect.max.y * pixels_per_point)
            .ceil()
            .clamp(0.0, screen_height as f32) as i32;
        let width = (max_x - min_x).max(0);
        let height = (max_y - min_y).max(0);
        if width == 0 || height == 0 {
            None
        } else {
            Some([min_x, min_y, width, height])
        }
    }
}

impl Drop for GlEguiPainter {
    fn drop(&mut self) {
        if !self.initialized {
            return;
        }
        for (_, texture) in self.textures.drain() {
            delete_texture(texture.texture);
        }
        for texture in self.icon_free_pool.drain(..) {
            delete_texture(texture);
        }
        for texture in self.hero_free_pool.drain(..) {
            delete_texture(texture);
        }
        unsafe {
            if self.white_texture != 0 {
                delete_texture(self.white_texture);
            }
            if self.vbo != 0 {
                glDeleteBuffers(1, &self.vbo);
            }
            if self.ibo != 0 {
                glDeleteBuffers(1, &self.ibo);
            }
            if self.program != 0 {
                glDeleteProgram(self.program);
            }
        }
    }
}

enum UploadOutcome {
    Done { created: bool },
    Deferred,
}

fn load_shader(type_: GLenum, binary: &[u8]) -> Result<GLuint> {
    unsafe {
        let shader = glCreateShader(type_);
        if shader == 0 { bail!("glCreateShader failed"); }
        vglShaderGxpBinary(1, &shader, binary.as_ptr().cast(), binary.len() as GLsizei);
        let mut status = 0;
        glGetShaderiv(shader, GL_COMPILE_STATUS, &mut status);
        let error = glGetError();
        if status == 0 || error != GL_NO_ERROR {
            glDeleteShader(shader);
            bail!("GXP shader load failed (status={status}, GL error=0x{error:X})");
        }
        Ok(shader)
    }
}

fn link_program(vertex_src: &[u8], fragment_src: &[u8]) -> Result<GLuint> {
    unsafe {
        let vs = load_shader(GL_VERTEX_SHADER, vertex_src)?;
        let fs = match load_shader(GL_FRAGMENT_SHADER, fragment_src) {
            Ok(shader) => shader,
            Err(error) => { glDeleteShader(vs); return Err(error); }
        };
        let program = glCreateProgram();
        if program == 0 {
            glDeleteShader(vs);
            glDeleteShader(fs);
            bail!("glCreateProgram failed");
        }
        glAttachShader(program, vs);
        glAttachShader(program, fs);
        glLinkProgram(program);
        glDeleteShader(vs);
        glDeleteShader(fs);
        let mut status = 0;
        glGetProgramiv(program, GL_LINK_STATUS, &mut status);
        if status == GL_FALSE as i32 {
            let log = program_info_log(program);
            glDeleteProgram(program);
            bail!("program link failed: {log}");
        }
        Ok(program)
    }
}

unsafe fn program_info_log(program: GLuint) -> String {
    unsafe {
        let mut length = 0;
        glGetProgramiv(program, GL_INFO_LOG_LENGTH, &mut length);
        if length <= 1 {
            return String::from("(empty log)");
        }
        let mut buf = vec![0u8; length as usize];
        let mut written = 0;
        glGetProgramInfoLog(
            program,
            length,
            &mut written,
            buf.as_mut_ptr() as *mut GLchar,
        );
        String::from_utf8_lossy(&buf[..written.max(0) as usize]).into_owned()
    }
}

fn create_empty_texture(width: u32, height: u32) -> Result<GLuint> {
    unsafe {
        let mut id = 0u32;
        glGenTextures(1, &mut id);
        if id == 0 {
            bail!("glGenTextures failed");
        }
        glBindTexture(GL_TEXTURE_2D, id);
        set_texture_params();
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            GL_RGBA as i32,
            width as i32,
            height as i32,
            0,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            ptr::null(),
        );
        glBindTexture(GL_TEXTURE_2D, 0);
        let err = glGetError();
        if err != GL_NO_ERROR {
            glDeleteTextures(1, &id);
            bail!("glTexImage2D failed with GL error 0x{err:X}");
        }
        Ok(id)
    }
}

fn upload_texture_region(
    texture: GLuint,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    pixels: &[u8],
    row_stride_pixels: usize,
) -> Result<()> {
    unsafe {
        glBindTexture(GL_TEXTURE_2D, texture);
        glTexSubImage2D(
            GL_TEXTURE_2D,
            0,
            x as i32,
            y as i32,
            width as i32,
            height as i32,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            pixels.as_ptr() as *const _,
        );
        glBindTexture(GL_TEXTURE_2D, 0);
        let err = glGetError();
        if err != GL_NO_ERROR {
            bail!("glTexSubImage2D failed with GL error 0x{err:X}");
        }
        let _ = row_stride_pixels;
        Ok(())
    }
}

unsafe fn set_texture_params() {
    unsafe {
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as i32);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as i32);
    }
}

fn delete_texture(texture: GLuint) {
    if texture == 0 {
        return;
    }
    unsafe {
        glDeleteTextures(1, &texture);
    }
}
