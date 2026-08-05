/// RAII guard around `SDL_StartTextInput`/`SDL_StopTextInput` (which wrap
/// `sceImeDialogInit`/`sceImeDialogTerm` on the Vita SDL2 backend). Without
/// this, an early return or `?` anywhere while the dialog is open leaves the
/// common dialog un-terminated, and the process exits with it still alive —
/// indistinguishable from a crash the moment the keyboard is opened.
pub struct ImeGuard<'a> {
    video: &'a sdl2::VideoSubsystem,
}

impl<'a> ImeGuard<'a> {
    pub fn open(video: &'a sdl2::VideoSubsystem) -> Self {
        video.text_input().start();
        Self { video }
    }
}

impl Drop for ImeGuard<'_> {
    fn drop(&mut self) {
        self.video.text_input().stop();
    }
}
