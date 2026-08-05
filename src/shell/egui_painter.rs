use anyhow::Result;
use std::collections::HashMap;

/// How many failed uploads to retry per frame. Retrying all of them at once
/// would just fail all of them again while video memory is still tight.
const RETRIES_PER_FRAME: usize = 2;

#[derive(Default)]
pub struct SdlEguiPainter {
    textures: HashMap<egui::TextureId, SdlEguiTexture>,
    /// Uploads that could not be allocated, kept so they can be retried.
    ///
    /// egui hands out each texture delta exactly once, and a mesh whose texture
    /// is missing is skipped entirely. Without this, a single failed allocation
    /// — which the console does hit when a lot of artwork is live at once —
    /// left that image blank for the rest of the session.
    pending: HashMap<egui::TextureId, PendingUpload>,
    vertices: Vec<sdl2::render::Vertex>,
    indices: Vec<i32>,
    /// Reused across every texture delta on the fast (non-retry) path — a
    /// fresh `Vec` + full pixel-by-pixel copy on every icon/font-atlas upload
    /// was a real per-frame cost during scroll bursts. Only the rare
    /// OOM-retry path (`PendingUpload::pixels`) still needs its own owned
    /// clone, since that one has to outlive this buffer being reused next call.
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
}

impl SdlEguiPainter {
    /// Whether any upload is still waiting for video memory. The frame loop
    /// keeps redrawing while this holds, since retries only happen while
    /// painting — going idle with something pending would strand it.
    pub fn has_pending_uploads(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn paint(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        screen_size: [u32; 2],
        pixels_per_point: f32,
        primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) -> Result<()> {
        self.apply_textures(canvas, textures_delta);

        for clipped_primitive in primitives {
            let Some(clip_rect) =
                Self::sdl_clip_rect(clipped_primitive.clip_rect, screen_size, pixels_per_point)
            else {
                continue;
            };
            canvas.set_clip_rect(clip_rect);

            let egui::epaint::Primitive::Mesh(mesh) = &clipped_primitive.primitive else {
                continue;
            };
            if mesh.indices.is_empty() || mesh.vertices.is_empty() {
                continue;
            }

            let texture = self.textures.get(&mesh.texture_id);

            if texture.is_none() && mesh.texture_id != egui::TextureId::default() {
                continue;
            }
            self.vertices.clear();
            let uv_scale = texture.map(|t| t.uv_scale).unwrap_or(egui::vec2(1.0, 1.0));
            self.vertices.extend(
                mesh.vertices
                    .iter()
                    .map(|vertex| Self::sdl_vertex(vertex, pixels_per_point, uv_scale)),
            );

            self.indices.clear();
            self.indices.extend(mesh.indices.iter().map(|&i| i as i32));

            let texture_ref = texture.map(|t| &t.texture);

            if let Err(err) = canvas.render_geometry(&self.vertices, texture_ref, &self.indices) {
                eprintln!("skipped a draw call: {err}");
            }
        }

        canvas.set_clip_rect(None);
        for texture_id in &textures_delta.free {
            self.textures.remove(texture_id);
            self.pending.remove(texture_id);
        }

        Ok(())
    }

    fn apply_textures(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        textures_delta: &egui::TexturesDelta,
    ) {
        // Give the images that ran out of video memory another go first —
        // eviction may have freed room since. These already own their pixel
        // buffer from a previous failed attempt, so no conversion needed.
        if !self.pending.is_empty() {
            let retry: Vec<egui::TextureId> =
                self.pending.keys().copied().take(RETRIES_PER_FRAME).collect();
            for texture_id in retry {
                let upload = self.pending.remove(&texture_id).expect("key came from the map");
                self.upload(canvas, texture_id, upload.size, upload.pos, &upload.pixels);
            }
        }

        // `scratch` is reused across every delta here instead of a fresh
        // `Vec` per image — a real cost during icon/cover-art bursts. Taken
        // out for the duration of the loop so `self.upload` (which needs
        // `&mut self` for `textures`/`pending`) doesn't alias it; given back
        // at the end so its allocation survives to the next frame.
        let mut scratch = std::mem::take(&mut self.scratch);
        for (texture_id, delta) in &textures_delta.set {
            scratch.clear();
            Self::fill_sdl_rgba(&delta.image, &mut scratch);
            self.upload(canvas, *texture_id, delta.image.size(), delta.pos, &scratch);
        }
        self.scratch = scratch;
    }

    /// Uploads one image, parking a copy of `pixels` in `pending` if video
    /// memory says no.
    fn upload(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        texture_id: egui::TextureId,
        size: [usize; 2],
        pos: Option<[usize; 2]>,
        pixels: &[u8],
    ) {
        use sdl2::pixels::PixelFormatEnum;
        use sdl2::rect::Rect;
        use sdl2::render::BlendMode;

        let [width, height] = size;

        if pos.is_none() || !self.textures.contains_key(&texture_id) {
            let texture =
                canvas.create_texture_streaming(PixelFormatEnum::RGBA32, width as u32, height as u32);
            let mut texture = match texture {
                Ok(texture) => texture,
                Err(err) => {
                    eprintln!("no room for a {width}x{height} texture, will retry: {err}");
                    self.pending.insert(texture_id, PendingUpload { size, pos, pixels: pixels.to_vec() });
                    return;
                }
            };
            texture.set_blend_mode(BlendMode::Blend);
            if let Err(err) = texture.update(Rect::new(0, 0, width as u32, height as u32), pixels, width * 4) {
                eprintln!("couldn't upload a texture, will retry: {err}");
                self.pending.insert(texture_id, PendingUpload { size, pos, pixels: pixels.to_vec() });
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

    /// Writes `image` into `out` as RGBA8, clearing it first. Takes an
    /// out-param rather than returning a fresh `Vec` so callers can reuse one
    /// allocation across many images instead of paying for one per delta.
    fn fill_sdl_rgba(image: &egui::ImageData, out: &mut Vec<u8>) {
        match image {
            egui::ImageData::Color(image) => {
                for pixel in &image.pixels {
                    out.extend_from_slice(&pixel.to_srgba_unmultiplied());
                }
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
        let [r, g, b, a] = vertex.color.to_srgba_unmultiplied();
        sdl2::render::Vertex {
            position: sdl2::rect::FPoint::new(
                vertex.pos.x * pixels_per_point,
                vertex.pos.y * pixels_per_point,
            ),
            color: sdl2::pixels::Color::RGBA(r, g, b, a),
            // Clamped because SDL validates texture coordinates and throws out
            // the *entire* draw call if any one of them lands outside [0, 1].
            // egui's own arithmetic — remapping a rounded rect's arc points into
            // the brush's UV range, or a computed crop — routinely lands on
            // -0.0000001, which was enough to drop every image on the screen.
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
        // The console's SDL rejects a whole draw call when any uv falls outside
        // [0, 1], and egui's arithmetic lands just outside it all the time —
        // this is what blanked every image on the detail screen.
        let (x, y) = SdlEguiPainter::uv_for_test(egui::vec2(-1e-7, 1.0000002), egui::vec2(1.0, 1.0));
        assert_eq!((x, y), (0.0, 1.0));
        assert!(x.is_sign_positive(), "a negative zero is still out of bounds for SDL");

        // Values already in range are passed through untouched.
        let (x, y) = SdlEguiPainter::uv_for_test(egui::vec2(0.25, 0.78), egui::vec2(1.0, 1.0));
        assert_eq!((x, y), (0.25, 0.78));
    }
}
