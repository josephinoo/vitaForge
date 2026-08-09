use anyhow::Result;
use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;
use std::collections::HashMap;

use crate::app::icons::MAX_ICON_SIDE;
const RETRIES_PER_FRAME: usize = 2;

const MAX_UPLOAD_ATTEMPTS: u32 = 8;
const BACKOFF_RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);
// New streaming-texture creation cost 30-160ms on real hardware (vs Vita3K, where it barely
const NEW_TEXTURES_PER_FRAME: usize = 1;
const ICON_FREE_POOL_CAP: usize = 32;
const ICON_FREE_POOL_WARM_LOW: usize = 8;
#[derive(Default)]
pub struct SdlEguiPainter {
    textures: HashMap<egui::TextureId, SdlEguiTexture>,
    pending: HashMap<egui::TextureId, PendingUpload>,
    icon_free_pool: Vec<sdl2::render::Texture>,
    vertices: Vec<sdl2::render::Vertex>,
    indices: Vec<i32>,
    scratch: Vec<u8>,
}
struct SdlEguiTexture {
    texture: sdl2::render::Texture,
    uv_scale: egui::Vec2,
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
}
impl SdlEguiPainter {
    pub fn paint(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        screen_size: [u32; 2],
        pixels_per_point: f32,
        primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) -> Result<PaintStats> {
        let texture_apply_started_at = std::time::Instant::now();
        let textures_uploaded = self.apply_textures(canvas, textures_delta);
        let texture_apply_secs = texture_apply_started_at.elapsed().as_secs_f64();
        let geometry_started_at = std::time::Instant::now();
        let mut draw_calls = 0u32;
        let mut vertices_drawn = 0u32;
        let mut current_clip: Option<sdl2::rect::Rect> = None;
        let mut current_texture_id: Option<egui::TextureId> = None;
        for clipped_primitive in primitives {
            let Some(clip_rect) =
                Self::sdl_clip_rect(clipped_primitive.clip_rect, screen_size, pixels_per_point)
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
                None if mesh.texture_id != egui::TextureId::default() => continue,
                None => egui::vec2(1.0, 1.0),
            };
            let same_batch = current_clip == Some(clip_rect) && current_texture_id == Some(mesh.texture_id);
            if !same_batch {
                self.flush_batch(canvas, current_texture_id, &mut draw_calls, &mut vertices_drawn);
                canvas.set_clip_rect(clip_rect);
                current_clip = Some(clip_rect);
                current_texture_id = Some(mesh.texture_id);
            }
            let base_index = self.vertices.len() as u32;
            self.vertices.extend(
                mesh.vertices
                    .iter()
                    .map(|vertex| Self::sdl_vertex(vertex, pixels_per_point, uv_scale)),
            );
            self.indices.extend(mesh.indices.iter().map(|&i| (base_index + i) as i32));
        }
        self.flush_batch(canvas, current_texture_id, &mut draw_calls, &mut vertices_drawn);
        let geometry_secs = geometry_started_at.elapsed().as_secs_f64();
        canvas.set_clip_rect(None);
        for texture_id in &textures_delta.free {
            self.pending.remove(texture_id);
            let Some(freed) = self.textures.remove(texture_id) else { continue };
            let query = freed.texture.query();
            if query.width == MAX_ICON_SIDE
                && query.height == MAX_ICON_SIDE
                && self.icon_free_pool.len() < ICON_FREE_POOL_CAP
            {
                self.icon_free_pool.push(freed.texture);
            }
        }
        Ok(PaintStats { texture_apply_secs, geometry_secs, draw_calls, textures_uploaded, vertices_drawn })
    }
    fn is_new_creation(&self, texture_id: egui::TextureId, pos: Option<[usize; 2]>) -> bool {
        pos.is_none() || !self.textures.contains_key(&texture_id)
    }
    fn is_icon_class_size(size: [usize; 2]) -> bool {
        size[0] as u32 <= MAX_ICON_SIDE && size[1] as u32 <= MAX_ICON_SIDE
    }
    fn flush_batch(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        texture_id: Option<egui::TextureId>,
        draw_calls: &mut u32,
        vertices_drawn: &mut u32,
    ) {
        if self.indices.is_empty() || self.vertices.is_empty() {
            self.vertices.clear();
            self.indices.clear();
            return;
        }
        let texture_ref = texture_id.and_then(|id| self.textures.get(&id)).map(|t| &t.texture);
        if let Err(err) = canvas.render_geometry(&self.vertices, texture_ref, &self.indices) {
            eprintln!("skipped a draw call: {err}");
        } else {
            *draw_calls += 1;
            *vertices_drawn += self.vertices.len() as u32;
        }
        self.vertices.clear();
        self.indices.clear();
    }
    fn apply_textures(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        textures_delta: &egui::TexturesDelta,
    ) -> u32 {
        let mut uploaded = 0u32;
        let mut new_creations = 0usize;
        if !self.pending.is_empty() {
            let now = std::time::Instant::now();
            let retry: Vec<egui::TextureId> = self
                .pending
                .iter()
                .filter(|(_, upload)| upload.next_retry_at <= now)
                .map(|(id, _)| *id)
                .take(RETRIES_PER_FRAME.min(NEW_TEXTURES_PER_FRAME) + 1)
                .collect();
            for texture_id in retry {
                let upload = self.pending.remove(&texture_id).expect("key came from the map");
                if Self::is_icon_class_size(upload.size) {
                    if !self.upload_icon(canvas, texture_id, upload.size, &upload.pixels) {
                        new_creations += 1;
                    }
                } else {
                    self.upload(canvas, texture_id, upload.size, upload.pos, &upload.pixels, upload.attempts);
                    new_creations += 1;
                }
                uploaded += 1;
            }
        }
        let mut scratch = std::mem::take(&mut self.scratch);
        for (texture_id, delta) in &textures_delta.set {
            scratch.clear();
            Self::fill_sdl_rgba(&delta.image, &mut scratch);
            let is_new = self.is_new_creation(*texture_id, delta.pos);
            if is_new && Self::is_icon_class_size(delta.image.size()) {
                let would_create = self.icon_free_pool.is_empty();
                if would_create && new_creations >= NEW_TEXTURES_PER_FRAME {
                    self.pending.insert(
                        *texture_id,
                        PendingUpload {
                            size: delta.image.size(),
                            pos: None,
                            pixels: scratch.clone(),
                            attempts: 0,
                            next_retry_at: std::time::Instant::now(),
                        },
                    );
                    continue;
                }
                if !self.upload_icon(canvas, *texture_id, delta.image.size(), &scratch) {
                    new_creations += 1;
                }
                uploaded += 1;
                continue;
            }
            if is_new && new_creations >= NEW_TEXTURES_PER_FRAME {
                self.pending.insert(
                    *texture_id,
                    PendingUpload {
                        size: delta.image.size(),
                        pos: delta.pos,
                        pixels: scratch.clone(),
                        attempts: 0,
                        next_retry_at: std::time::Instant::now(),
                    },
                );
                continue;
            }
            if is_new {
                new_creations += 1;
            }
            self.upload(canvas, *texture_id, delta.image.size(), delta.pos, &scratch, 0);
            uploaded += 1;
        }
        self.scratch = scratch;
        if new_creations == 0 && self.icon_free_pool.len() < ICON_FREE_POOL_WARM_LOW {
            if let Ok(mut texture) =
                canvas.create_texture_streaming(PixelFormatEnum::RGBA32, MAX_ICON_SIDE, MAX_ICON_SIDE)
            {
                texture.set_blend_mode(BlendMode::Blend);
                self.icon_free_pool.push(texture);
            }
        }
        uploaded
    }
    fn upload_icon(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pixels: &[u8],
    ) -> bool {
        if let Some(texture) = self.icon_free_pool.pop() {
            self.finish_icon_upload(texture, texture_id, size, pixels);
            return true;
        }
        match canvas.create_texture_streaming(PixelFormatEnum::RGBA32, MAX_ICON_SIDE, MAX_ICON_SIDE) {
            Ok(mut texture) => {
                texture.set_blend_mode(BlendMode::Blend);
                self.finish_icon_upload(texture, texture_id, size, pixels);
            }
            Err(err) => {
                eprintln!("no room for a {MAX_ICON_SIDE}x{MAX_ICON_SIDE} icon texture, will retry: {err}");
                self.defer_or_give_up(texture_id, size, None, pixels, 0);
            }
        }
        false
    }
    fn finish_icon_upload(
        &mut self,
        mut texture: sdl2::render::Texture,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pixels: &[u8],
    ) {
        let [width, height] = size;
        if let Err(err) = texture.update(Rect::new(0, 0, width as u32, height as u32), pixels, width * 4) {
            eprintln!("couldn't patch a pooled icon texture, will retry: {err}");
            if self.icon_free_pool.len() < ICON_FREE_POOL_CAP {
                self.icon_free_pool.push(texture);
            }
            self.defer_or_give_up(texture_id, size, None, pixels, 0);
            return;
        }
        let cap = MAX_ICON_SIDE as f32;
        let uv_scale = egui::vec2(width as f32 / cap, height as f32 / cap);
        self.textures.insert(texture_id, SdlEguiTexture { texture, uv_scale });
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
        if attempts >= MAX_UPLOAD_ATTEMPTS {
            eprintln!("giving up on a {}x{} texture after {attempts} attempts", size[0], size[1]);
            return;
        }
        self.pending.insert(
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
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pos: Option<[usize; 2]>,
        pixels: &[u8],
        attempts: u32,
    ) {
        let [width, height] = size;
        if pos.is_none() || !self.textures.contains_key(&texture_id) {
            let texture =
                canvas.create_texture_streaming(PixelFormatEnum::RGBA32, width as u32, height as u32);
            let mut texture = match texture {
                Ok(texture) => texture,
                Err(err) => {
                    eprintln!("no room for a {width}x{height} texture, will retry: {err}");
                    self.defer_or_give_up(texture_id, size, pos, pixels, attempts);
                    return;
                }
            };
            texture.set_blend_mode(BlendMode::Blend);
            if let Err(err) = texture.update(Rect::new(0, 0, width as u32, height as u32), pixels, width * 4) {
                eprintln!("couldn't upload a texture, will retry: {err}");
                self.defer_or_give_up(texture_id, size, pos, pixels, attempts);
                return;
            }
            self.textures.insert(texture_id, SdlEguiTexture { texture, uv_scale: egui::vec2(1.0, 1.0) });
            return;
        }
        let Some([x, y]) = pos else {
            eprintln!("partial texture update with no position, skipped");
            return;
        };
        let Some(existing) = self.textures.get_mut(&texture_id) else {
            eprintln!("partial update for a texture that no longer exists, skipped");
            return;
        };
        if let Err(err) =
            existing.texture.update(Rect::new(x as i32, y as i32, width as u32, height as u32), pixels, width * 4)
        {
            eprintln!("couldn't patch a texture: {err}");
        }
    }
    fn fill_sdl_rgba(image: &egui::ImageData, out: &mut Vec<u8>) {
        match image {
            egui::ImageData::Color(image) => {

                let bytes: &[u8] = unsafe {
                    std::slice::from_raw_parts(image.pixels.as_ptr() as *const u8, image.pixels.len() * 4)
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
    fn sdl_vertex(
        vertex: &egui::epaint::Vertex,
        pixels_per_point: f32,
        uv_scale: egui::Vec2,
    ) -> sdl2::render::Vertex {
        let [r, g, b, a] = vertex.color.to_array();
        sdl2::render::Vertex {
            position: sdl2::rect::FPoint::new(
                vertex.pos.x * pixels_per_point,
                vertex.pos.y * pixels_per_point,
            ),
            color: sdl2::pixels::Color::RGBA(r, g, b, a),
            tex_coord: sdl2::rect::FPoint::new(
                (vertex.uv.x * uv_scale.x).clamp(0.0, 1.0),
                (vertex.uv.y * uv_scale.y).clamp(0.0, 1.0),
            ),
        }
    }
    #[cfg(test)]
    pub(crate) fn uv_for_test(uv: egui::Vec2, uv_scale: egui::Vec2) -> (f32, f32) {
        let vertex = egui::epaint::Vertex {
            pos: egui::Pos2::ZERO,
            uv: uv.to_pos2(),
            color: egui::Color32::WHITE,
        };
        let v = Self::sdl_vertex(&vertex, 1.0, uv_scale);
        (v.tex_coord.x(), v.tex_coord.y())
    }
    fn sdl_clip_rect(
        clip_rect: egui::Rect,
        [screen_width, screen_height]: [u32; 2],
        pixels_per_point: f32,
    ) -> Option<sdl2::rect::Rect> {
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
        let width = (max_x - min_x).max(0) as u32;
        let height = (max_y - min_y).max(0) as u32;
        if width == 0 || height == 0 {
            None
        } else {
            Some(sdl2::rect::Rect::new(min_x, min_y, width, height))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn texture_coordinates_stay_inside_the_range_sdl_accepts() {
        let (x, y) = SdlEguiPainter::uv_for_test(egui::vec2(-1e-7, 1.0000002), egui::vec2(1.0, 1.0));
        assert_eq!((x, y), (0.0, 1.0));
        assert!(x.is_sign_positive(), "a negative zero is still out of bounds for SDL");
        let (x, y) = SdlEguiPainter::uv_for_test(egui::vec2(0.25, 0.78), egui::vec2(1.0, 1.0));
        assert_eq!((x, y), (0.25, 0.78));
    }
}
