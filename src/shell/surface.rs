use crate::shell::egui_painter::SdlEguiPainter;
use anyhow::{Context, Result};
use sdl2::render::Canvas;
use sdl2::video::Window;

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

        Ok(Self { canvas, egui_painter: SdlEguiPainter::default() })
    }

    #[allow(dead_code)]
    pub fn window(&self) -> &Window {
        self.canvas.window()
    }

    pub fn draw_scene(&mut self) {
        self.canvas.set_clip_rect(None);
        self.canvas.set_draw_color(sdl2::pixels::Color::BLACK);
        self.canvas.clear();
    }

    pub fn paint_egui(
        &mut self,
        pixels_per_point: f32,
        primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) -> Result<()> {
        self.egui_painter.paint(&mut self.canvas, [WIDTH, HEIGHT], pixels_per_point, primitives, textures_delta)?;
        self.canvas.present();
        Ok(())
    }

    pub fn snapshot_egui(
        &mut self,
        pixels_per_point: f32,
        primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) -> Result<()> {
        self.draw_scene();
        self.paint_egui(pixels_per_point, primitives, textures_delta)
    }

    #[allow(dead_code)]
    pub fn present_snapshot(&mut self) -> Result<()> {
        self.canvas.present();
        Ok(())
    }
}
