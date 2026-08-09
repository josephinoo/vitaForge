use anyhow::Result;
use std::collections::HashMap;

#[derive(Default)]
pub struct SdlEguiPainter {
    textures: HashMap<egui::TextureId, SdlEguiTexture>,
    vertices: Vec<sdl2::render::Vertex>,
    indices: Vec<i32>,
}

struct SdlEguiTexture {
    texture: sdl2::render::Texture,
    uv_scale: egui::Vec2,
}

impl SdlEguiPainter {
    pub fn paint(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        screen_size: [u32; 2],
        pixels_per_point: f32,
        primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) -> Result<()> {
        self.apply_textures(canvas, textures_delta);

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
                self.flush_batch(canvas, current_texture_id);
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

        self.flush_batch(canvas, current_texture_id);

        canvas.set_clip_rect(None);
        for texture_id in &textures_delta.free {
            self.textures.remove(texture_id);
        }

        Ok(())
    }

    fn flush_batch(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        texture_id: Option<egui::TextureId>,
    ) {
        if self.indices.is_empty() || self.vertices.is_empty() {
            self.vertices.clear();
            self.indices.clear();
            return;
        }

        let texture_ref = texture_id.and_then(|id| self.textures.get(&id)).map(|t| &t.texture);

        if let Err(err) = canvas.render_geometry(&self.vertices, texture_ref, &self.indices) {
            eprintln!("skipped a draw call: {err}");
        }

        self.vertices.clear();
        self.indices.clear();
    }

    fn apply_textures(
        &mut self,
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        textures_delta: &egui::TexturesDelta,
    ) {
        for (texture_id, delta) in &textures_delta.set {
            Self::upload_texture(canvas, &mut self.textures, *texture_id, delta);
        }
    }

    fn upload_texture(
        canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        textures: &mut HashMap<egui::TextureId, SdlEguiTexture>,
        texture_id: egui::TextureId,
        delta: &egui::epaint::ImageDelta,
    ) {
        use sdl2::pixels::PixelFormatEnum;
        use sdl2::rect::Rect;
        use sdl2::render::BlendMode;

        let [width, height] = delta.image.size();
        let pixels = Self::image_to_sdl_rgba(&delta.image);

        if delta.pos.is_none() || !textures.contains_key(&texture_id) {
            let texture = canvas.create_texture_streaming(PixelFormatEnum::RGBA32, width as u32, height as u32);
            let mut texture = match texture {
                Ok(texture) => texture,
                Err(err) => {
                    eprintln!("couldn't allocate a texture ({width}x{height}): {err}");
                    return;
                }
            };
            texture.set_blend_mode(BlendMode::Blend);
            if let Err(err) = texture.update(Rect::new(0, 0, width as u32, height as u32), &pixels, width * 4) {
                eprintln!("couldn't upload a texture: {err}");
                return;
            }
            textures.insert(texture_id, SdlEguiTexture { texture, uv_scale: egui::vec2(1.0, 1.0) });
            return;
        }

        let Some(&[x, y]) = delta.pos.as_ref() else {
            eprintln!("partial texture update with no position, skipped");
            return;
        };
        let Some(existing) = textures.get_mut(&texture_id) else {
            eprintln!("partial update for a texture that no longer exists, skipped");
            return;
        };
        if let Err(err) = existing.texture.update(Rect::new(x as i32, y as i32, width as u32, height as u32), &pixels, width * 4) {
            eprintln!("couldn't patch a texture: {err}");
        }
    }

    fn image_to_sdl_rgba(image: &egui::ImageData) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(image.width() * image.height() * 4);
        match image {
            egui::ImageData::Color(image) => {
                for pixel in &image.pixels {
                    pixels.extend_from_slice(&pixel.to_srgba_unmultiplied());
                }
            }
            egui::ImageData::Font(image) => {
                for pixel in image.srgba_pixels(None) {
                    pixels.extend_from_slice(&pixel.to_srgba_unmultiplied());
                }
            }
        }
        pixels
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
            tex_coord: sdl2::rect::FPoint::new(vertex.uv.x * uv_scale.x, vertex.uv.y * uv_scale.y),
        }
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
