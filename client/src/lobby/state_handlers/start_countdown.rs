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
    if let Some(assets) = assets {
        let (wall_texture, sky_texture) = match game_data.difficulty {
            2 => {
                let sky_texture = Some(assets.blue_rust_texture.clone());
                let wall_texture = &assets.bull_texture;
                (wall_texture, sky_texture)
            }
            3 => {
                let sky_texture = Some(assets.brown_rust_texture.clone());
                let wall_texture = &assets.dolphins_texture;
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
            &assets.floor_texture,
        ));

        ClientState::Lobby(Lobby::Countdown {
            end_time,
            game_data,
            maze_meshes,
            sky_mesh,
        })
    } else {
        let sky_colors = sky::sky_colors(game_data.difficulty);
        let sky_mesh = sky::generate_sky(None, sky_colors);

        ClientState::Lobby(Lobby::Countdown {
            end_time,
            game_data,
            maze_meshes: None,
            sky_mesh,
        })
    }
}
