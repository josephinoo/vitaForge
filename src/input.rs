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

#[derive(Debug, Clone, PartialEq)]
pub enum AppCommand {
    Input(InputCommand),
    SetSearchQuery(String),
    RequestSearch,
    CloseSearch,
    SetCategoryFilter(Option<crate::data::Category>),
    SetSortOrder(crate::data::SortOrder),
    SelectApp { index: usize, origin: Option<egui::Rect> },
    BackToCatalog,
    InstallCurrent,
    DismissInstall,
    SelfUpdate,
    Exit,
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
        Button::Back => return Some(AppCommand::Exit),
        Button::DPadUp => InputCommand::MoveUp,
        Button::DPadDown => InputCommand::MoveDown,
        Button::DPadLeft => InputCommand::MoveLeft,
        Button::DPadRight => InputCommand::MoveRight,
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
    if y.abs() >= x.abs() {
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

pub fn map_pointer_event(
    event: &Event,
    screen_size: (f32, f32),
    pixels_per_point: f32,
    pointer_pos: &mut egui::Pos2,
) -> Option<egui::Event> {
    match *event {
        Event::MouseMotion { x, y, .. } => {
            *pointer_pos = mouse_to_screen_pos(x, y, pixels_per_point);
            Some(egui::Event::PointerMoved(*pointer_pos))
        }
        Event::MouseButtonDown { mouse_btn, x, y, .. } => Some(pointer_button_at(
            pointer_pos,
            mouse_to_screen_pos(x, y, pixels_per_point),
            map_mouse_button(mouse_btn),
            true,
        )),
        Event::MouseButtonUp { mouse_btn, x, y, .. } => Some(pointer_button_at(
            pointer_pos,
            mouse_to_screen_pos(x, y, pixels_per_point),
            map_mouse_button(mouse_btn),
            false,
        )),
        Event::FingerDown { touch_id, x, y, .. } if touch_id == FRONT_TOUCH_DEVICE_ID => {
            Some(pointer_button_at(pointer_pos, touch_to_screen_pos(x, y, screen_size), egui::PointerButton::Primary, true))
        }
        Event::FingerUp { touch_id, x, y, .. } if touch_id == FRONT_TOUCH_DEVICE_ID => {
            Some(pointer_button_at(pointer_pos, touch_to_screen_pos(x, y, screen_size), egui::PointerButton::Primary, false))
        }
        Event::FingerMotion { touch_id, x, y, .. } if touch_id == FRONT_TOUCH_DEVICE_ID => {
            *pointer_pos = touch_to_screen_pos(x, y, screen_size);
            Some(egui::Event::PointerMoved(*pointer_pos))
        }
        _ => None,
    }
}

fn pointer_button_at(pointer_pos: &mut egui::Pos2, pos: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
    *pointer_pos = pos;
    egui::Event::PointerButton { pos, button, pressed, modifiers: egui::Modifiers::default() }
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
