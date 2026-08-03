use crate::shell::egui_painter::SdlEguiPainter;
use anyhow::{Context, Result};
use sdl2::render::{Canvas, Texture};
use sdl2::video::Window;

pub const WIDTH: u32 = 960;
pub const HEIGHT: u32 = 544;

pub struct VitaSurface {
    canvas: Canvas<Window>,
    egui_painter: SdlEguiPainter,

    frame_snapshot: Option<Texture>,
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

        Ok(Self { canvas, egui_painter: SdlEguiPainter::default(), frame_snapshot: None })
    }

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
        use sdl2::pixels::PixelFormatEnum;
        use sdl2::render::{BlendMode, TextureAccess};

        if self.frame_snapshot.is_none() {
            match self.canvas.create_texture(PixelFormatEnum::RGBA32, TextureAccess::Target, WIDTH, HEIGHT) {
                Ok(mut created) => {
                    created.set_blend_mode(BlendMode::None);
                    self.frame_snapshot = Some(created);
                }
                Err(err) => eprintln!("couldn't create the keyboard snapshot texture: {err}"),
            }
        }

        let Self { canvas, egui_painter, frame_snapshot } = self;
        let Some(texture) = frame_snapshot else {
            let _ = egui_painter.paint(canvas, [WIDTH, HEIGHT], pixels_per_point, primitives, textures_delta);
            canvas.present();
            return Ok(());
        };

        let rendered = canvas.with_texture_canvas(texture, |canvas| {
            canvas.set_clip_rect(None);
            canvas.set_draw_color(sdl2::pixels::Color::BLACK);
            canvas.clear();
            let _ = egui_painter.paint(canvas, [WIDTH, HEIGHT], pixels_per_point, primitives, textures_delta);
        });
        if let Err(err) = rendered {
            eprintln!("couldn't render into the keyboard snapshot: {err}");
        }

        self.present_snapshot()
    }

    pub fn present_snapshot(&mut self) -> Result<()> {
        self.canvas.set_clip_rect(None);
        self.canvas.set_draw_color(sdl2::pixels::Color::BLACK);
        self.canvas.clear();
        if let Some(texture) = &self.frame_snapshot {
            self.canvas
                .copy(texture, None, None)
                .map_err(anyhow::Error::msg)
                .context("failed to blit the IME snapshot")?;
        }
        self.canvas.present();
        Ok(())
    }
}
