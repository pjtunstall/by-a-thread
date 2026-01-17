use crate::{
    assets::Assets,
    game::world::maze,
    game::world::sky,
    state::{ClientState, Lobby},
};
use common::snapshot::InitialData;

pub fn handle_countdown_started(
    end_time: f64,
    game_data: InitialData,
    assets: Option<&Assets>,
) -> ClientState {
    let assets = assets.expect("assets required for countdown but none provided");
    let (wall_texture, sky_texture) = match game_data.difficulty {
        2 => {
            let sky_texture = Some(assets.blue_rust_texture.clone());
            let wall_texture = &assets.bull_texture;
            (wall_texture, sky_texture)
        }
        3 => {
            let sky_texture = None;
            let wall_texture = &assets.white_rust_texture;
            (wall_texture, sky_texture)
        }
        _ => {
            let sky_texture = None;
            let wall_texture = &assets.griffin_texture;
            (wall_texture, sky_texture)
        }
    };

    let sky_colors = sky::sky_colors(game_data.difficulty);
    let sky_mesh = sky::generate_sky(sky_texture, sky_colors);
    let maze_meshes = Some(maze::build_maze_meshes(
        &game_data.maze,
        wall_texture,
        game_data.difficulty,
    ));

    ClientState::Lobby(Lobby::Countdown {
        end_time,
        game_data,
        maze_meshes,
        sky_mesh,
    })
}
