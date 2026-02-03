use crate::{
    assets::Assets,
    game::world::maze,
    game::world::sky,
    info,
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
        0 => {
            let sky_texture = Some(assets.purple_texture.clone());
            let wall_texture = &assets.griffin_texture;
            (wall_texture, sky_texture)
        }
        1 => {
            let sky_texture = Some(sky::generate_starfield_texture());
            let wall_texture = &assets.ants_texture;
            (wall_texture, sky_texture)
        }
        2 => {
            let sky_texture = Some(assets.white_marble_texture.clone());
            let wall_texture = &assets.happy_monkeys_texture;
            (wall_texture, sky_texture)
        }
        3 => {
            let sky_texture = Some(assets.dolphins_texture.clone());
            let wall_texture = &assets.dolphins_texture;
            (wall_texture, sky_texture)
        }
        4 => {
            let sky_texture = Some(assets.blue_rust_texture.clone());
            let wall_texture = &assets.bull_texture;
            (wall_texture, sky_texture)
        }
        5 => {
            let sky_texture = Some(assets.white_marble_texture.clone());
            let wall_texture = &assets.squids_texture;
            (wall_texture, sky_texture)
        }
        6 => {
            let sky_texture = Some(assets.white_marble_texture.clone());
            let wall_texture = &assets.circuits_texture;
            (wall_texture, sky_texture)
        }
        7 => {
            let sky_texture = Some(assets.green_marble_texture.clone());
            let wall_texture = &assets.sad_monkeys_texture;
            (wall_texture, sky_texture)
        }
        8 => {
            let sky_texture = Some(assets.white_marble_texture.clone());
            let wall_texture = &assets.ants_in_maze_texture;
            (wall_texture, sky_texture)
        }
        9 => {
            let sky_texture = Some(sky::generate_starfield_texture());
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

    let maze_meshes = maze::build_maze_meshes(&game_data.maze, wall_texture, game_data.difficulty);
    let map_overlay = info::map::initialize_map(&game_data.maze, &assets.map_font);

    ClientState::Lobby(Lobby::Countdown {
        end_time,
        game_data,
        maze_meshes: Some(maze_meshes),
        map_overlay: Some(map_overlay),
        sky_mesh,
    })
}
