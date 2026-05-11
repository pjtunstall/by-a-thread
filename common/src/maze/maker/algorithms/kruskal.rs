use disjoint::DisjointSetVec;
use rand::seq::SliceRandom;

use super::super::MazeMaker;

pub trait Kruskal {
    fn kruskal(&mut self);
}

impl Kruskal for MazeMaker {
    fn kruskal(&mut self) {
        let (rooms, mut walls, _pillars, room_to_index) = self.get_rooms_walls_pillars();

        for room in &rooms {
            self.grid[room.z][room.x] = 0;
        }

        let mut rooms = DisjointSetVec::from(rooms);

        walls.shuffle(&mut rand::rng());
        for wall in walls {
            let (room_1, room_2) = self.get_flanking_cells(wall);
            let i = room_to_index.get(&[room_1.x, room_1.z]).expect(&format!(
                "room x={}, z={} not in `room_to_index` `HashMap`, flanking wall: x={}, z={}, {:#?}",
                room_1.x, room_1.z, wall.x, wall.z, wall.orientation
            ));
            let j = room_to_index.get(&[room_2.x, room_2.z]).expect(&format!(
                "room x={}, z={} not in `room_to_index` `HashMap`, flanking wall: x={}, z={}, {:#?}",
                room_2.x, room_2.z, wall.x, wall.z, wall.orientation
            ));
            if rooms.root_of(*i) != rooms.root_of(*j) {
                rooms.join(*i, *j);
                self.grid[wall.z][wall.x] = 0;
            }
        }
    }
}
