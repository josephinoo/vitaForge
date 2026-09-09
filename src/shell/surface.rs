use anyhow::{Context, Result};
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

#[cfg(target_os = "vita")]
pub struct VitaSurface {
    window: Window,
    egui_painter: crate::shell::gl_egui_painter::GlEguiPainter,
    has_common_dialog: bool,
}

#[cfg(not(target_os = "vita"))]
pub struct VitaSurface {
    egui_painter: crate::shell::egui_painter::SdlEguiPainter,
    canvas: sdl2::render::Canvas<Window>,
}

#[cfg(target_os = "vita")]
impl VitaSurface {
    pub fn new(video: &sdl2::VideoSubsystem) -> Result<Self> {
        use crate::shell::vgl::{
            GL_FALSE, SCE_GXM_MULTISAMPLE_NONE, vglInitExtended, vglSetCircularPoolSize,
            vglSwapBuffers,
        };

        crate::shell::vgl_splash_stub::retain_splash_stubs();
        crate::install::log_file("boot: vglInitExtended begin");
        unsafe {
            vglSetCircularPoolSize(3 * 1024);
            let resolution_fallback = vglInitExtended(
                0,
                WIDTH as i32,
                HEIGHT as i32,
                0x0180_0000,
                SCE_GXM_MULTISAMPLE_NONE,
            );
            if resolution_fallback != GL_FALSE {
                anyhow::bail!("vitaGL could not use the required 960x544 resolution");
            }
        }
        crate::install::log_file("vitaGL: vglInitExtended ok (VitaDB-compatible)");

        let mut egui_painter = crate::shell::gl_egui_painter::GlEguiPainter::new();
        crate::install::log_file("boot: GL painter init");
        egui_painter
            .init(WIDTH, HEIGHT)
            .context("failed to init GL egui painter (shader compile/link)")?;
        egui_painter.prewarm();
        crate::install::log_file("vitaGL: egui painter shaders ready");

        let window = video
            .window("vitaForge", WIDTH, HEIGHT)
            .position_centered()
            .build()
            .context("failed to create SDL window")?;

        egui_painter.clear();
        unsafe {
            vglSwapBuffers(GL_FALSE);
        }

        Ok(Self {
            window,
            egui_painter,
            has_common_dialog: false,
        })
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn set_common_dialog(&mut self, active: bool) {
        self.has_common_dialog = active;
    }

    pub fn draw_scene(&mut self) {
        self.egui_painter.clear();
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
        use crate::shell::gl_egui_painter::PaintStats;
        use crate::shell::vgl::{GL_FALSE, GL_TRUE, vglSwapBuffers};

        let PaintStats {
            texture_apply_secs,
            geometry_secs,
            draw_calls,
            textures_uploaded,
            vertices_drawn,
            missing_textures,
        } = self.egui_painter.paint(
            [WIDTH, HEIGHT],
            pixels_per_point,
            primitives,
            textures_delta,
        )?;
        let present_started_at = std::time::Instant::now();
        let flag = if self.has_common_dialog {
            GL_TRUE
        } else {
            GL_FALSE
        };
        unsafe {
            vglSwapBuffers(flag);
        }
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

    pub fn set_frame_alpha(&mut self, alpha: f32) {
        self.egui_painter.set_frame_alpha(alpha);
    }
}

#[cfg(not(target_os = "vita"))]
impl VitaSurface {
    pub fn new(video: &sdl2::VideoSubsystem) -> Result<Self> {
        use crate::shell::egui_painter::SdlEguiPainter;

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
        Ok(Self {
            egui_painter,
            canvas,
        })
    }

    pub fn window(&self) -> &Window {
        self.canvas.window()
    }

    pub fn set_common_dialog(&mut self, _active: bool) {}

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
        use crate::shell::egui_painter::PaintStats;

        let PaintStats {
            texture_apply_secs,
            geometry_secs,
            draw_calls,
            textures_uploaded,
            vertices_drawn,
            missing_textures,
        } = self.egui_painter.paint(
            &mut self.canvas,
            [WIDTH, HEIGHT],
            pixels_per_point,
            primitives,
            textures_delta,
        )?;
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

    pub fn set_frame_alpha(&mut self, _alpha: f32) {}
}
