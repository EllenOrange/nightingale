use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;

const STICK_DEADZONE: f32 = 0.5;

#[derive(Resource, Default)]
pub struct NavInput {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub confirm: bool,
    pub back: bool,

    pub up_held: bool,
    pub down_held: bool,
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NavInput>()
            .init_resource::<StickRepeat>()
            .add_systems(PreUpdate, update_nav_input.after(bevy::input::gamepad::gamepad_event_processing_system));
    }
}

#[derive(Resource)]
struct StickRepeat {
    timer: Timer,
    axis: Option<StickAxis>,
    started: bool,
}

#[derive(PartialEq, Clone, Copy)]
enum StickAxis {
    Up,
    Down,
    Left,
    Right,
}

impl Default for StickRepeat {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.4, TimerMode::Once),
            axis: None,
            started: false,
        }
    }
}

fn update_nav_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut nav: ResMut<NavInput>,
    mut stick_rep: ResMut<StickRepeat>,
    time: Res<Time>,
) {
    let mut gp_up = false;
    let mut gp_down = false;
    let mut gp_left = false;
    let mut gp_right = false;
    let mut gp_confirm = false;
    let mut gp_back = false;
    let mut gp_up_held = false;
    let mut gp_down_held = false;

    let mut stick_dir: Option<StickAxis> = None;

    for gamepad in &gamepads {
        gp_up |= gamepad.just_pressed(GamepadButton::DPadUp);
        gp_down |= gamepad.just_pressed(GamepadButton::DPadDown);
        gp_left |= gamepad.just_pressed(GamepadButton::DPadLeft);
        gp_right |= gamepad.just_pressed(GamepadButton::DPadRight);
        gp_confirm |= gamepad.just_pressed(GamepadButton::South);
        gp_back |= gamepad.just_pressed(GamepadButton::East)
            || gamepad.just_pressed(GamepadButton::Start);

        gp_up_held |= gamepad.pressed(GamepadButton::DPadUp);
        gp_down_held |= gamepad.pressed(GamepadButton::DPadDown);

        let stick = gamepad.left_stick();
        if stick.y > STICK_DEADZONE {
            stick_dir = Some(StickAxis::Up);
        } else if stick.y < -STICK_DEADZONE {
            stick_dir = Some(StickAxis::Down);
        } else if stick.x < -STICK_DEADZONE {
            stick_dir = Some(StickAxis::Left);
        } else if stick.x > STICK_DEADZONE {
            stick_dir = Some(StickAxis::Right);
        }
    }

    let mut stick_up = false;
    let mut stick_down = false;
    let mut stick_left = false;
    let mut stick_right = false;
    let mut stick_up_held = false;
    let mut stick_down_held = false;

    if let Some(dir) = stick_dir {
        match dir {
            StickAxis::Up => { stick_up_held = true; }
            StickAxis::Down => { stick_down_held = true; }
            _ => {}
        }

        if stick_rep.axis != Some(dir) {
            stick_rep.axis = Some(dir);
            stick_rep.timer = Timer::from_seconds(0.4, TimerMode::Once);
            stick_rep.started = true;
            match dir {
                StickAxis::Up => stick_up = true,
                StickAxis::Down => stick_down = true,
                StickAxis::Left => stick_left = true,
                StickAxis::Right => stick_right = true,
            }
        } else if stick_rep.started {
            stick_rep.timer.tick(time.delta());
            if stick_rep.timer.just_finished() {
                stick_rep.timer = Timer::from_seconds(0.08, TimerMode::Repeating);
                match dir {
                    StickAxis::Up => stick_up = true,
                    StickAxis::Down => stick_down = true,
                    StickAxis::Left => stick_left = true,
                    StickAxis::Right => stick_right = true,
                }
            }
        }
    } else {
        stick_rep.axis = None;
        stick_rep.started = false;
    }

    nav.up = keyboard.just_pressed(KeyCode::ArrowUp) || gp_up || stick_up;
    nav.down = keyboard.just_pressed(KeyCode::ArrowDown) || gp_down || stick_down;
    nav.left = keyboard.just_pressed(KeyCode::ArrowLeft) || gp_left || stick_left;
    nav.right = keyboard.just_pressed(KeyCode::ArrowRight) || gp_right || stick_right;
    nav.confirm = keyboard.just_pressed(KeyCode::Enter) || gp_confirm;
    nav.back = keyboard.just_pressed(KeyCode::Escape) || gp_back;
    nav.up_held = keyboard.pressed(KeyCode::ArrowUp) || gp_up_held || stick_up_held;
    nav.down_held = keyboard.pressed(KeyCode::ArrowDown) || gp_down_held || stick_down_held;
}
