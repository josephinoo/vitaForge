use sdl2::controller::{Axis, Button, GameController};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputCommand {
    Back,
    Confirm,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    CategoryPrev,
    CategoryNext,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTarget {
    Search,
    Comment,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StoreTab {
    #[default]
    Discover,
    Library,
    Updates,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverRail {
    Top,
    New,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppCommand {
    Input(InputCommand),
    SetSearchQuery(String),
    RequestSearch,
    CloseSearch,
    SetCategoryFilter(Option<crate::data::Category>),
    SetGenreFilter(Option<String>),
    SetSourceFilter(Option<crate::data::SourceCatalog>),
    SetSortOrder(crate::data::SortOrder),
    FlipSortDirection,
    SelectApp { index: usize },
    SelectAppById(String),
    SetStoreTab(StoreTab),
    SeeAllRail(DiscoverRail),
    BackToDiscoverHome,
    MoreByAuthor(String),
    OpenScreenshot(usize),
    CloseScreenshot,
    BackToCatalog,
    InstallCurrent,
    DismissInstall,
    CancelInstall,
    CancelDataPrompt,
    OpenSettings,
    CloseSettings,
    SetLanguage(crate::app::i18n::Language),
    ToggleInstallNotifications,
    ClearIconCache,
    ClearCatalogCache,
    PurgeAllCache,
    ToggleLike,
    RateCurrent(u8),
    RequestCommentEntry,
    CloseCommentEntry,
    SubmitComment(String),
    SelfUpdate,
}
impl From<InputCommand> for AppCommand {
    fn from(cmd: InputCommand) -> Self {
        AppCommand::Input(cmd)
    }
}
pub const FRONT_TOUCH_DEVICE_ID: i64 = 1;
pub fn map_keyboard_event(event: &Event) -> Option<AppCommand> {
    let Event::KeyDown { keycode: Some(key), repeat: false, .. } = event else {
        return None;
    };
    let command = match *key {
        Keycode::Escape => InputCommand::Back,
        Keycode::Return => InputCommand::Confirm,
        Keycode::Up => InputCommand::MoveUp,
        Keycode::Down => InputCommand::MoveDown,
        Keycode::Left => InputCommand::MoveLeft,
        Keycode::Right => InputCommand::MoveRight,
        Keycode::Q | Keycode::PageUp => InputCommand::CategoryPrev,
        Keycode::E | Keycode::PageDown => InputCommand::CategoryNext,
        Keycode::F | Keycode::Slash => return Some(AppCommand::RequestSearch),
        Keycode::L => return Some(AppCommand::ToggleLike),
        Keycode::C => return Some(AppCommand::RequestCommentEntry),
        Keycode::S => return Some(AppCommand::FlipSortDirection),
        _ => return None,
    };
    Some(command.into())
}
pub fn map_controller_button_event(event: &Event) -> Option<AppCommand> {
    let Event::ControllerButtonDown { button, .. } = event else {
        return None;
    };
    let command = match button {
        Button::A => InputCommand::Confirm,
        Button::B => InputCommand::Back,
        Button::Y => return Some(AppCommand::RequestSearch),
        Button::Start => return Some(AppCommand::OpenSettings),
        Button::X => return Some(AppCommand::ToggleLike),
        Button::Back => return Some(AppCommand::FlipSortDirection),
        Button::LeftShoulder => InputCommand::CategoryPrev,
        Button::RightShoulder => InputCommand::CategoryNext,
        _ => return None,
    };
    Some(command.into())
}
const STICK_DEADZONE: f32 = 0.6;
pub fn held_stick_direction(controller: Option<&GameController>) -> Option<InputCommand> {
    let controller = controller?;
    let x = controller.axis(Axis::LeftX) as f32 / i16::MAX as f32;
    let y = controller.axis(Axis::LeftY) as f32 / i16::MAX as f32;
    let from_stick = if y.abs() >= x.abs() {
        match y {
            y if y <= -STICK_DEADZONE => Some(InputCommand::MoveUp),
            y if y >= STICK_DEADZONE => Some(InputCommand::MoveDown),
            _ => None,
        }
    } else {
        match x {
            x if x <= -STICK_DEADZONE => Some(InputCommand::MoveLeft),
            x if x >= STICK_DEADZONE => Some(InputCommand::MoveRight),
            _ => None,
        }
    };
    from_stick.or_else(|| held_dpad_direction(controller))
}
fn held_dpad_direction(controller: &GameController) -> Option<InputCommand> {
    if controller.button(Button::DPadUp) {
        Some(InputCommand::MoveUp)
    } else if controller.button(Button::DPadDown) {
        Some(InputCommand::MoveDown)
    } else if controller.button(Button::DPadLeft) {
        Some(InputCommand::MoveLeft)
    } else if controller.button(Button::DPadRight) {
        Some(InputCommand::MoveRight)
    } else {
        None
    }
}
pub fn register_vita_controller_mapping(sdl: &sdl2::Sdl) -> Result<(), String> {
    let joystick = sdl.joystick()?;
    if joystick.num_joysticks().map_err(|e| e.to_string())? == 0 {
        return Ok(());
    }
    let guid = joystick.device_guid(0).map_err(|e| e.to_string())?;
    let mapping = format!(
        "{guid},PSVita Controller,\
         a:b2,b:b1,x:b3,y:b0,\
         back:b10,start:b11,\
         leftshoulder:b4,rightshoulder:b5,\
         leftstick:b14,rightstick:b15,\
         dpup:b8,dpdown:b6,dpleft:b7,dpright:b9,\
         leftx:a0,lefty:a1,rightx:a2,righty:a3,\
         lefttrigger:b12,righttrigger:b13,platform:PS Vita,"
    );
    sdl.game_controller()?.add_mapping(&mapping).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn open_first_controller(subsystem: &sdl2::GameControllerSubsystem) -> Option<GameController> {
    let available = subsystem.num_joysticks().ok()?;
    (0..available).find_map(|id| subsystem.is_game_controller(id).then(|| subsystem.open(id).ok())?)
}
pub struct Pointer {
    pos: egui::Pos2,
    front_touch: i64,
    active_finger: Option<i64>,
    saw_finger: bool,
    release_pending: bool,
    cancel_pending: bool,
}
impl Pointer {
    pub fn new() -> Self {
        Self::for_touch_device(front_touch_device().unwrap_or(FRONT_TOUCH_DEVICE_ID))
    }
    fn for_touch_device(front_touch: i64) -> Self {
        Self {
            pos: egui::Pos2::ZERO,
            front_touch,
            active_finger: None,
            saw_finger: false,
            release_pending: false,
            cancel_pending: false,
        }
    }
    pub fn deferred(&mut self, out: &mut Vec<egui::Event>) {
        if std::mem::take(&mut self.cancel_pending) {
            let pos = self.pos;
            out.push(self.button_at(pos, egui::PointerButton::Primary, false));
        }
        if std::mem::take(&mut self.release_pending) {
            out.push(egui::Event::PointerGone);
        }
    }
    pub fn map_event(
        &mut self,
        event: &Event,
        screen_size: (f32, f32),
        pixels_per_point: f32,
        out: &mut Vec<egui::Event>,
    ) {
        match *event {
            Event::MouseMotion { which, x, y, .. } if !self.is_emulated(which) => {
                self.pos = mouse_to_screen_pos(x, y, pixels_per_point);
                out.push(egui::Event::PointerMoved(self.pos));
            }
            Event::MouseButtonDown { which, mouse_btn, x, y, .. } if !self.is_emulated(which) => {
                let pos = mouse_to_screen_pos(x, y, pixels_per_point);
                out.push(self.button_at(pos, map_mouse_button(mouse_btn), true));
            }
            Event::MouseButtonUp { which, mouse_btn, x, y, .. } if !self.is_emulated(which) => {
                let pos = mouse_to_screen_pos(x, y, pixels_per_point);
                out.push(self.button_at(pos, map_mouse_button(mouse_btn), false));
            }
            Event::FingerDown { touch_id, finger_id, x, y, .. } if self.is_front(touch_id) => {
                if self.active_finger.is_some() {
                    return; // a second finger must not yank the pointer away from the one dragging
                }
                self.active_finger = Some(finger_id);
                self.saw_finger = true;
                self.release_pending = false;
                let pos = touch_to_screen_pos(x, y, screen_size);
                self.pos = pos;
                out.push(egui::Event::PointerMoved(pos));
                out.push(self.button_at(pos, egui::PointerButton::Primary, true));
            }
            Event::FingerUp { touch_id, finger_id, x, y, .. }
                if self.is_front(touch_id) && self.active_finger == Some(finger_id) =>
            {
                self.active_finger = None;
                self.release_pending = true;
                let pos = touch_to_screen_pos(x, y, screen_size);
                out.push(self.button_at(pos, egui::PointerButton::Primary, false));
            }
            Event::FingerMotion { touch_id, finger_id, x, y, .. }
                if self.is_front(touch_id) && self.active_finger == Some(finger_id) =>
            {
                self.pos = touch_to_screen_pos(x, y, screen_size);
                out.push(egui::Event::PointerMoved(self.pos));
            }
            _ => {}
        }
    }
    pub fn forget_touch(&mut self) {
        self.cancel_pending = self.active_finger.take().is_some();
        self.release_pending = true;
    }
    fn is_front(&self, touch_id: i64) -> bool {
        touch_id == self.front_touch
    }
    fn is_emulated(&self, which: u32) -> bool {
        which == SDL_TOUCH_MOUSEID || self.saw_finger
    }
    fn button_at(&mut self, pos: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
        self.pos = pos;
        egui::Event::PointerButton { pos, button, pressed, modifiers: egui::Modifiers::default() }
    }
}
impl Default for Pointer {
    fn default() -> Self {
        Self::new()
    }
}
const SDL_TOUCH_MOUSEID: u32 = u32::MAX;
fn front_touch_device() -> Option<i64> {
    unsafe {
        let count = sdl2::sys::SDL_GetNumTouchDevices();
        (0..count).map(|index| sdl2::sys::SDL_GetTouchDevice(index)).find(|&id| {
            sdl2::sys::SDL_GetTouchDeviceType(id)
                == sdl2::sys::SDL_TouchDeviceType::SDL_TOUCH_DEVICE_DIRECT
        })
    }
}
fn mouse_to_screen_pos(x: i32, y: i32, pixels_per_point: f32) -> egui::Pos2 {
    egui::pos2(x as f32 / pixels_per_point, y as f32 / pixels_per_point)
}
fn touch_to_screen_pos(x: f32, y: f32, screen_size: (f32, f32)) -> egui::Pos2 {
    egui::pos2(x * screen_size.0, y * screen_size.1)
}
fn map_mouse_button(button: MouseButton) -> egui::PointerButton {
    match button {
        MouseButton::Right => egui::PointerButton::Secondary,
        MouseButton::Middle => egui::PointerButton::Middle,
        _ => egui::PointerButton::Primary,
    }
}

#[cfg(test)]
mod tests {
    use super::{Pointer, SDL_TOUCH_MOUSEID};
    use sdl2::event::Event;
    const SCREEN: (f32, f32) = (960.0, 544.0);
    fn finger(kind: u8, finger_id: i64, x: f32, y: f32) -> Event {
        match kind {
            0 => Event::FingerDown { timestamp: 0, touch_id: 7, finger_id, x, y, dx: 0.0, dy: 0.0, pressure: 1.0 },
            1 => Event::FingerMotion { timestamp: 0, touch_id: 7, finger_id, x, y, dx: 0.0, dy: 0.0, pressure: 1.0 },
            _ => Event::FingerUp { timestamp: 0, touch_id: 7, finger_id, x, y, dx: 0.0, dy: 0.0, pressure: 1.0 },
        }
    }
    fn drain(pointer: &mut Pointer, event: &Event) -> Vec<egui::Event> {
        let mut out = Vec::new();
        pointer.map_event(event, SCREEN, 1.0, &mut out);
        out
    }
    #[test]
    fn a_press_moves_before_it_presses() {
        let mut pointer = Pointer::for_touch_device(7);
        let events = drain(&mut pointer, &finger(0, 1, 0.5, 0.5));
        assert!(matches!(events[0], egui::Event::PointerMoved(pos) if pos == egui::pos2(480.0, 272.0)));
        assert!(matches!(events[1], egui::Event::PointerButton { pressed: true, .. }));
    }
    #[test]
    fn a_second_finger_does_not_steal_the_drag() {
        let mut pointer = Pointer::for_touch_device(7);
        drain(&mut pointer, &finger(0, 1, 0.5, 0.5));
        assert!(drain(&mut pointer, &finger(0, 2, 0.1, 0.1)).is_empty());
        assert!(drain(&mut pointer, &finger(1, 2, 0.2, 0.2)).is_empty());
        assert!(!drain(&mut pointer, &finger(1, 1, 0.6, 0.6)).is_empty());
    }
    #[test]
    fn the_rear_pad_is_not_a_pointer() {
        let mut pointer = Pointer::for_touch_device(0);
        assert!(drain(&mut pointer, &finger(0, 1, 0.5, 0.5)).is_empty());
    }
    #[test]
    fn the_pointer_leaves_a_frame_after_the_finger_lifts() {
        let mut pointer = Pointer::for_touch_device(7);
        drain(&mut pointer, &finger(0, 1, 0.5, 0.5));
        let up = drain(&mut pointer, &finger(2, 1, 0.5, 0.5));
        assert!(matches!(up[..], [egui::Event::PointerButton { pressed: false, .. }]));
        let mut deferred = Vec::new();
        pointer.deferred(&mut deferred);
        assert!(matches!(deferred[..], [egui::Event::PointerGone]));
        let mut again = Vec::new();
        pointer.deferred(&mut again);
        assert!(again.is_empty());
    }
    #[test]
    fn touch_emulated_mouse_events_are_dropped() {
        let mut pointer = Pointer::for_touch_device(7);
        let emulated = Event::MouseMotion {
            timestamp: 0,
            window_id: 0,
            which: SDL_TOUCH_MOUSEID,
            mousestate: sdl2::mouse::MouseState::from_sdl_state(0),
            x: 10,
            y: 10,
            xrel: 0,
            yrel: 0,
        };
        assert!(drain(&mut pointer, &emulated).is_empty());
        let real = Event::MouseMotion {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mousestate: sdl2::mouse::MouseState::from_sdl_state(0),
            x: 10,
            y: 10,
            xrel: 0,
            yrel: 0,
        };
        assert!(!drain(&mut pointer, &real).is_empty());
        drain(&mut pointer, &finger(0, 1, 0.5, 0.5));
        assert!(drain(&mut pointer, &real).is_empty()); // real fingers are in play now
    }
}
