use std::collections::HashMap;
use std::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sfx {
    Navigate,
    ShowModal,
    HideModal,
    ToggleOn,
    ToggleOff,
    Launch,
    IntoDetail,
    OutOfDetail,
    MenuFlyIn,
    MenuFlyOut,
    TabTransition,
    Activation,
    Typing,
}

struct WavSound {
    sample_rate: u32,
    channels: u16,
    samples: Vec<i16>,
}

impl WavSound {
    fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return None;
        }
        let mut pos = 12;
        let mut fmt_found = false;
        let mut channels = 2u16;
        let mut sample_rate = 44100u32;

        while pos + 8 <= bytes.len() {
            let chunk_id = &bytes[pos..pos + 4];
            let chunk_size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
            pos += 8;

            if chunk_id == b"fmt " && chunk_size >= 16 && pos + chunk_size <= bytes.len() {
                channels = u16::from_le_bytes(bytes[pos + 2..pos + 4].try_into().ok()?);
                sample_rate = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?);
                fmt_found = true;
            } else if chunk_id == b"data" && pos + chunk_size <= bytes.len() {
                if !fmt_found {
                    return None;
                }
                let pcm_bytes = &bytes[pos..pos + chunk_size];
                let mut samples = Vec::with_capacity(pcm_bytes.len() / 2);
                for chunk in pcm_bytes.chunks_exact(2) {
                    samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
                }
                return Some(WavSound { sample_rate, channels, samples });
            }
            pos += chunk_size;
        }
        None
    }
}

pub struct AudioEngine {
    tx: mpsc::Sender<Sfx>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Sfx>();
        std::thread::spawn(move || {
            let mut sounds = HashMap::new();
            load(&mut sounds, Sfx::Navigate, include_bytes!("../assets/sounds/deck_ui_navigation.wav"));
            load(&mut sounds, Sfx::ShowModal, include_bytes!("../assets/sounds/deck_ui_show_modal.wav"));
            load(&mut sounds, Sfx::HideModal, include_bytes!("../assets/sounds/deck_ui_hide_modal.wav"));
            load(&mut sounds, Sfx::ToggleOn, include_bytes!("../assets/sounds/deck_ui_switch_toggle_on.wav"));
            load(&mut sounds, Sfx::ToggleOff, include_bytes!("../assets/sounds/deck_ui_switch_toggle_off.wav"));
            load(&mut sounds, Sfx::Launch, include_bytes!("../assets/sounds/deck_ui_launch_game.wav"));
            load(&mut sounds, Sfx::IntoDetail, include_bytes!("../assets/sounds/deck_ui_into_game_detail.wav"));
            load(&mut sounds, Sfx::OutOfDetail, include_bytes!("../assets/sounds/deck_ui_out_of_game_detail.wav"));
            load(&mut sounds, Sfx::MenuFlyIn, include_bytes!("../assets/sounds/deck_ui_side_menu_fly_in.wav"));
            load(&mut sounds, Sfx::MenuFlyOut, include_bytes!("../assets/sounds/deck_ui_side_menu_fly_out.wav"));
            load(&mut sounds, Sfx::TabTransition, include_bytes!("../assets/sounds/deck_ui_tab_transition_01.wav"));
            load(&mut sounds, Sfx::Activation, include_bytes!("../assets/sounds/deck_ui_default_activation.wav"));
            load(&mut sounds, Sfx::Typing, include_bytes!("../assets/sounds/deck_ui_typing.wav"));

            // The Vita only allows a small, slowly-reclaimed pool of hardware audio ports
            // system-wide. All bundled sound effects share the same format (48kHz mono), so
            // open one port for the app's whole lifetime instead of per-play — opening and
            // releasing a port on every single sound effect exhausts the pool after a handful
            // of plays and audio goes silent for good.
            let format = sounds
                .get(&Sfx::Navigate)
                .map(|s| (s.sample_rate, s.channels))
                .unwrap_or((48_000, 1));
            let port = open_port(format.0, format.1);

            while let Ok(effect) = rx.recv() {
                if let Some(sound) = sounds.get(&effect) {
                    play_sound_on_hardware(port, sound);
                }
            }
            close_port(port);
        });
        AudioEngine { tx }
    }

    pub fn play(&self, effect: Sfx) {
        let _ = self.tx.send(effect);
    }
}

fn load(map: &mut HashMap<Sfx, WavSound>, effect: Sfx, bytes: &[u8]) {
    if let Some(sound) = WavSound::parse(bytes) {
        map.insert(effect, sound);
    }
}

const GRAIN_SIZE: usize = 512;

#[cfg(target_os = "vita")]
fn open_port(sample_rate: u32, channels: u16) -> i32 {
    use vitasdk_sys::*;
    let mode = if channels == 1 { SCE_AUDIO_OUT_MODE_MONO } else { SCE_AUDIO_OUT_MODE_STEREO };
    unsafe { sceAudioOutOpenPort(SCE_AUDIO_OUT_PORT_TYPE_MAIN, GRAIN_SIZE as i32, sample_rate as i32, mode as u32) }
}

#[cfg(not(target_os = "vita"))]
fn open_port(_sample_rate: u32, _channels: u16) -> i32 {
    -1
}

#[cfg(target_os = "vita")]
fn close_port(port: i32) {
    if port >= 0 {
        unsafe {
            vitasdk_sys::sceAudioOutReleasePort(port);
        }
    }
}

#[cfg(not(target_os = "vita"))]
fn close_port(_port: i32) {}

#[cfg(target_os = "vita")]
fn play_sound_on_hardware(port: i32, sound: &WavSound) {
    if port < 0 {
        return;
    }
    unsafe {
        let channels = sound.channels.max(1) as usize;
        let sample_stride = channels * GRAIN_SIZE;
        for chunk in sound.samples.chunks(sample_stride) {
            if chunk.len() == sample_stride {
                vitasdk_sys::sceAudioOutOutput(port, chunk.as_ptr() as *const _);
            } else {
                let mut padded = vec![0i16; sample_stride];
                padded[..chunk.len()].copy_from_slice(chunk);
                vitasdk_sys::sceAudioOutOutput(port, padded.as_ptr() as *const _);
            }
        }
    }
}

#[cfg(not(target_os = "vita"))]
fn play_sound_on_hardware(_port: i32, _sound: &WavSound) {}
