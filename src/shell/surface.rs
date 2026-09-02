use crate::shell::egui_painter::{PaintStats, SdlEguiPainter};
use anyhow::{Context, Result};
use sdl2::render::Canvas;
use sdl2::video::Window;

#[derive(Default, Clone, Copy)]
pub struct FramePaintStats {
    pub texture_apply_secs: f64,
    pub geometry_secs: f64,
    pub present_secs: f64,
    pub draw_calls: u32,
    pub textures_uploaded: u32,
    pub vertices_drawn: u32,
    pub missing_textures: u32,
}
pub const WIDTH: u32 = 960;
pub const HEIGHT: u32 = 544;
pub struct VitaSurface {
    canvas: Canvas<Window>,
    egui_painter: SdlEguiPainter,
}
impl VitaSurface {
    pub fn new(video: &sdl2::VideoSubsystem) -> Result<Self> {
        sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1");
        let window = video
            .window("vitaForge", WIDTH, HEIGHT)
            .position_centered()
            .build()
            .context("failed to create SDL window")?;
        let mut canvas = window
            .into_canvas()
            .accelerated()
            .present_vsync()
            .build()
            .map_err(anyhow::Error::msg)
            .context("failed to create SDL renderer")?;
        canvas
            .set_logical_size(WIDTH, HEIGHT)
            .map_err(anyhow::Error::msg)
            .context("failed to set logical render size")?;
        let mut egui_painter = SdlEguiPainter::default();
        egui_painter.prewarm(&mut canvas);
        Ok(Self { canvas, egui_painter })
    }
    pub fn window(&self) -> &Window {
        self.canvas.window()
    }
    pub fn draw_scene(&mut self) {
        self.canvas.set_clip_rect(None);
        self.canvas.set_draw_color(sdl2::pixels::Color::BLACK);
        self.canvas.clear();
    }
    pub fn pending_texture_uploads(&self) -> usize {
        self.egui_painter.pending_uploads()
    }
    pub fn take_dropped_textures(&mut self) -> Vec<egui::TextureId> {
        self.egui_painter.take_dropped_textures()
    }
    pub fn force_recover(&mut self) -> Vec<egui::TextureId> {
        self.egui_painter.force_recover()
    }
    pub fn paint_egui(
        &mut self,
        pixels_per_point: f32,
        primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) -> Result<FramePaintStats> {
        let PaintStats { texture_apply_secs, geometry_secs, draw_calls, textures_uploaded, vertices_drawn, missing_textures } =
            self.egui_painter.paint(&mut self.canvas, [WIDTH, HEIGHT], pixels_per_point, primitives, textures_delta)?;
        let present_started_at = std::time::Instant::now();
        self.canvas.present();
        let present_secs = present_started_at.elapsed().as_secs_f64();
        Ok(FramePaintStats {
            texture_apply_secs,
            geometry_secs,
            present_secs,
            draw_calls,
            textures_uploaded,
            vertices_drawn,
            missing_textures,
        })
    }

}
