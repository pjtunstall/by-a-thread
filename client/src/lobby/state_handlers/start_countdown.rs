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
    
    let maze_meshes = maze::build_maze_meshes(
        &game_data.maze,
        wall_texture,
        game_data.difficulty,
    );

    let mut maze_escape = game_data.maze.clone();
    let (exit_z, exit_x) = game_data.exit_coords;
    maze_escape.grid[exit_z][exit_x] = 0;
    maze_escape.spaces.push(game_data.exit_coords);

    let maze_meshes_escape = maze::build_maze_meshes(
        &maze_escape,
        wall_texture,
        game_data.difficulty,
    );

    let map_overlay = info::map::initialize_map(&game_data.maze, &assets.map_font);
    let map_overlay_escape = info::map::initialize_map(&maze_escape, &assets.map_font);

    ClientState::Lobby(Lobby::Countdown {
        end_time,
        game_data,
        maze_meshes: Some(maze_meshes),
        maze_meshes_escape: Some(maze_meshes_escape),
        map_overlay: Some(map_overlay),
        map_overlay_escape: Some(map_overlay_escape),
        sky_mesh,
    })
}
