use macroquad::prelude::*;

use common::player::PlayerInput;

#[derive(Default, Clone, Copy)]
struct WasdKeysPrev {
    w: bool,
    s: bool,
    a: bool,
    d: bool,
}

#[derive(Clone, Copy)]
enum VerticalChoice {
    Forward,
    Back,
}

#[derive(Clone, Copy)]
enum StrafeChoice {
    Left,
    Right,
}

#[derive(Default)]
pub struct WasdOpposingResolver {
    prev: WasdKeysPrev,
    vertical: Option<VerticalChoice>,
    strafe: Option<StrafeChoice>,
}

pub fn player_input_from_keys(sim_tick: u64, resolver: &mut WasdOpposingResolver) -> PlayerInput {
    let w = is_key_down(KeyCode::W);
    let s = is_key_down(KeyCode::S);
    let a = is_key_down(KeyCode::A);
    let d = is_key_down(KeyCode::D);

    if w && s {
        let w_new = w && !resolver.prev.w;
        let s_new = s && !resolver.prev.s;
        if w_new && !s_new {
            resolver.vertical = Some(VerticalChoice::Forward);
        } else if s_new && !w_new {
            resolver.vertical = Some(VerticalChoice::Back);
        } else if w_new && s_new {
            resolver.vertical = Some(VerticalChoice::Forward);
        }
    } else {
        resolver.vertical = None;
    }

    if a && d {
        let a_new = a && !resolver.prev.a;
        let d_new = d && !resolver.prev.d;
        if a_new && !d_new {
            resolver.strafe = Some(StrafeChoice::Left);
        } else if d_new && !a_new {
            resolver.strafe = Some(StrafeChoice::Right);
        } else if a_new && d_new {
            resolver.strafe = Some(StrafeChoice::Left);
        }
    } else {
        resolver.strafe = None;
    }

    let (forward, backward) = match (w, s) {
        (true, false) => (true, false),
        (false, true) => (false, true),
        (false, false) => (false, false),
        (true, true) => match resolver.vertical {
            Some(VerticalChoice::Forward) => (true, false),
            Some(VerticalChoice::Back) => (false, true),
            None => (false, false),
        },
    };

    let (left, right) = match (a, d) {
        (true, false) => (true, false),
        (false, true) => (false, true),
        (false, false) => (false, false),
        (true, true) => match resolver.strafe {
            Some(StrafeChoice::Left) => (true, false),
            Some(StrafeChoice::Right) => (false, true),
            None => (false, false),
        },
    };

    resolver.prev = WasdKeysPrev { w, s, a, d };

    PlayerInput {
        sim_tick,
        forward,
        backward,
        left,
        right,
        yaw_left: is_key_down(KeyCode::Left),
        yaw_right: is_key_down(KeyCode::Right),
        pitch_up: is_key_down(KeyCode::Up),
        pitch_down: is_key_down(KeyCode::Down),
        fire: is_key_down(KeyCode::Space),
        fire_nonce: None,
        is_zoomed: is_key_down(KeyCode::LeftShift),
    }
}
