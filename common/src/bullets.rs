use glam::{Vec3, vec3};

use crate::{
    constants::{TICK_SECS, TICK_SECS_F32},
    maze::{CELL_SIZE, Maze},
    player,
};

pub const MAX_BULLETS_PER_PLAYER: usize = 32;
pub const FIRE_COOLDOWN_SECS: f64 = 0.1;
pub const SPEED: f32 = 720.0;
pub const LIFESPAN_SECS: f64 = 2.5;
pub const MAX_BOUNCES: u8 = 5;
pub const BULLET_SPAWN_OFFSET: f32 = player::RADIUS + BULLET_CORE_RADIUS + 0.1;

// The bullet's shell radius is used for display and for collisions with walls
// and floor. It's core radius is for collisions with players. This is to let
// the target feel undue danger to make the game more exciting; and for the sake
// of the visual effect of large bouncing bullets. The bet here is that the
// target's relief at surviving being clipped by a large sphere will outweigh
// any potential feeling the shooter might have that its periphery should be
// doing more damage.
pub const BULLET_SHELL_RADIUS: f32 = 4.0;
const BULLET_CORE_RADIUS: f32 = 0.1;

struct TestFaceParams {
    direction: Vec3,
    previous_position: Vec3,
    trace_distance: f32,
    box_min: Vec3,
    box_max: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bullet {
    pub id: u32,
    pub shooter_index: usize,
    pub position: Vec3,
    pub velocity: Vec3,
    pub spawn_tick: u64,
    pub bounces: u8,
}

impl Bullet {
    pub fn new(
        id: u32,
        shooter_index: usize,
        position: Vec3,
        velocity: Vec3,
        spawn_tick: u64,
    ) -> Self {
        Self {
            id,
            shooter_index,
            position,
            velocity,
            spawn_tick,
            bounces: 0,
        }
    }

    pub fn advance(&mut self, ticks: u64) {
        let delta = ticks as f32 * TICK_SECS_F32;
        self.position += self.velocity * delta;
    }

    pub fn is_expired(&self, current_tick: u64) -> bool {
        let age = (current_tick - self.spawn_tick) as f64 * TICK_SECS;
        age > LIFESPAN_SECS
    }

    pub fn has_bounced_enough(&self) -> bool {
        self.bounces > MAX_BOUNCES
    }

    pub fn redirect(&mut self, normal: Vec3) {
        redirect(&mut self.velocity, &mut self.bounces, normal);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallBounce {
    None,
    Bounce,
    Stuck,
}

pub fn bounce_off_ground(position: &mut Vec3, velocity: &mut Vec3, bounces: &mut u8) -> bool {
    if position.y > BULLET_SHELL_RADIUS || velocity.y >= 0.0 {
        return false;
    }

    // Time to impact: negative value, represents time in the past. Due to the
    // early returns, we can be sure that `t` <= 0.0.
    let t = (BULLET_SHELL_RADIUS - position.y) / velocity.y;

    // Rewind to impact point.
    *position += *velocity * t;

    // Reflect velocity.
    velocity.y *= -1.0;
    *bounces += 1;

    // Move the bullet further along its trajectory by the distance it traveled
    // underground.
    *position -= *velocity * t;

    return true;
}

// See Shirley et al.: Ray Tracing: The Next Weekend, Section 3.3 (axis aligned
// bounding boxes).
fn _find_intersection_with_box(
    previous_position: Vec3,
    ray_direction: Vec3,
    box_min: Vec3,
    box_max: Vec3,
) -> Option<f32> {
    // The master "start time". This rises as we find slabs that push the entry
    // point further back.
    let mut t_min: f32 = 0.0;

    // t_max: The master "end time." This shrinks as we find slabs that cut the
    // exit point shorter.
    let mut t_max: f32 = f32::MAX;

    // Check each axis.
    for i in 0..3 {
        // If the ray is parallel to an axis but outside its range, it doesn't
        // intersect the box.
        if ray_direction[i].abs() < 1e-6 {
            if previous_position[i] < box_min[i] || previous_position[i] > box_max[i] {
                return None;
            }
            // The ray is parallel and inside this axis range, so we continue.
        } else {
            // The distances along the ray to the two planes bounding the box
            // along this axis.
            let t1 = (box_min[i] - previous_position[i]) / ray_direction[i];
            let t2 = (box_max[i] - previous_position[i]) / ray_direction[i];

            // Where the ray intersects with the closest and furthest planes for the
            // current axis respectively.
            let t_enter = t1.min(t2);
            let t_exit = t1.max(t2);

            t_min = t_min.max(t_enter); // Latest entry time so far.
            t_max = t_max.min(t_exit); // Earliest exit time so far.

            // The intersection interval is empty; the ray leaves one axis slab
            // before entering another.
            if t_min > t_max {
                return None;
            }
        }
    }

    Some(t_min)
}

pub fn bounce_off_wall(
    position: &mut Vec3,
    velocity: &mut Vec3,
    bounces: &mut u8,
    maze: &Maze,
) -> WallBounce {
    let is_bullet_above_wall_height = position.y - BULLET_SHELL_RADIUS > CELL_SIZE;
    if is_bullet_above_wall_height {
        return WallBounce::None;
    }

    let direction = velocity.normalize_or_zero();
    if direction == Vec3::ZERO {
        return WallBounce::Stuck;
    }

    let trace_distance = velocity.length() * TICK_SECS_F32;
    let previous_position = *position - direction * trace_distance;
    let mut closest_hit: Option<(f32, Vec3)> = None;
    let end_pos = *position;

    // Get the min and max grid coordinates of the bullet's path this tick.
    let min_x = previous_position.x.min(end_pos.x) / CELL_SIZE;
    let max_x = previous_position.x.max(end_pos.x) / CELL_SIZE;
    let min_z = previous_position.z.min(end_pos.z) / CELL_SIZE;
    let max_z = previous_position.z.max(end_pos.z) / CELL_SIZE;

    // Check all grid cells along the path.
    for check_z in (min_z.floor() as isize - 1)..=(max_z.ceil() as isize + 1) {
        for check_x in (min_x.floor() as isize - 1)..=(max_x.ceil() as isize + 1) {
            if check_x < 0
                || check_z < 0
                || check_x >= maze.grid[0].len() as isize
                || check_z >= maze.grid.len() as isize
            {
                continue;
            }

            if maze.grid[check_z as usize][check_x as usize] == 0 {
                continue; // Empty cell.
            }

            // Opposite corners of this wall cell in world coordinates.
            let box_min = vec3(check_x as f32 * CELL_SIZE, 0.0, check_z as f32 * CELL_SIZE);
            let box_max = box_min + vec3(CELL_SIZE, CELL_SIZE, CELL_SIZE);

            // Check each face of the current wall cell. Update `closest_hit` if
            // we find a closer one.
            // West face.
            let test_face_params = TestFaceParams {
                direction,
                previous_position,
                trace_distance,
                box_min,
                box_max,
            };
            test_face(
                Vec3::new(-1.0, 0.0, 0.0),
                -box_min.x,
                &mut closest_hit,
                &test_face_params,
            );
            // East face.
            test_face(
                Vec3::new(1.0, 0.0, 0.0),
                box_max.x,
                &mut closest_hit,
                &test_face_params,
            );
            // North face.
            test_face(
                Vec3::new(0.0, 0.0, -1.0),
                -box_min.z,
                &mut closest_hit,
                &test_face_params,
            );
            // South face.
            test_face(
                Vec3::new(0.0, 0.0, 1.0),
                box_max.z,
                &mut closest_hit,
                &test_face_params,
            );
        }
    }

    if let Some((s, normal)) = closest_hit {
        // Move center back along its original path to exact sphere–plane contact.
        let hit_center = previous_position + direction * s;
        *position = hit_center;
        redirect(velocity, bounces, normal);

        // Let the bullet bounce off the wall by the distance that it would
        // otherwise have penetrated.
        let remaining = trace_distance - s;
        if remaining > 0.0 && velocity.length_squared() > 0.0 {
            let new_dir = velocity.normalize();
            *position += new_dir * remaining;
        }

        return WallBounce::Bounce;
    }

    WallBounce::None
}

// `test_face` checks one face of a wall cell (given its normal and plane constant) and, if it
// yields a closer valid hit along the bullet's path, updates `closest_hit`.
fn test_face(
    normal: Vec3,
    plane_const: f32,
    closest_hit: &mut Option<(f32, Vec3)>,
    test_face_params: &TestFaceParams,
) {
    let TestFaceParams {
        direction,
        previous_position,
        trace_distance,
        box_min,
        box_max,
    } = test_face_params;

    let denominator = direction.dot(normal);
    if denominator >= 0.0 {
        return; // Moving away or parallel, no hit.
    }

    // Ensure we start outside this face in the direction of travel.

    // `start_dist` is the signed distance from the bullet center to the face's infinite plane, measured along the face normal, at the start of the tick.
    let start_dist = previous_position.dot(normal) - plane_const;
    if start_dist <= BULLET_SHELL_RADIUS {
        // The bullet was already penetrating the wall or on the far side of it.
        return;
    }

    // Solve `previous_position + s * direction` * normal = plane_const +
    // `BULLET_SHELL_RADIUS` for `s`.
    let numerator = plane_const + BULLET_SHELL_RADIUS - previous_position.dot(normal);
    let s = numerator / denominator;
    if s < 0.0 || s > *trace_distance {
        // Hit would be before or after this tick.
        return;
    }

    // Compute center of the bullet at the moment of contact with the wall face.
    let hit_center = *previous_position + *direction * s;

    // Discard this face if the sphere center, at its first contact time with this
    // plane, would be outside the finite rectangle of this box face.
    if normal.x.abs() > 0.5 {
        // X face: constrain y and z within box.
        if hit_center.y < box_min.y || hit_center.y > box_max.y {
            return;
        }
        if hit_center.z < box_min.z || hit_center.z > box_max.z {
            return;
        }
    } else {
        // Z face: constrain y and x within box.
        if hit_center.y < box_min.y || hit_center.y > box_max.y {
            return;
        }
        if hit_center.x < box_min.x || hit_center.x > box_max.x {
            return;
        }
    }

    if let Some((best_s, _)) = closest_hit {
        if s >= *best_s {
            return;
        }
    }

    *closest_hit = Some((s, normal));
}

pub fn is_bullet_colliding_with_player(bullet_position: Vec3, player_position: Vec3) -> bool {
    bullet_position.distance(player_position) < BULLET_CORE_RADIUS + player::RADIUS
}

pub fn direction_from_yaw_pitch(yaw: f32, pitch: f32) -> Vec3 {
    let direction = vec3(
        -yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    );
    direction.normalize_or_zero()
}

pub fn spawn_position(player_position: Vec3, direction: Vec3) -> Vec3 {
    player_position + direction * BULLET_SPAWN_OFFSET
}

pub fn cooldown_ticks() -> u64 {
    (FIRE_COOLDOWN_SECS / TICK_SECS).ceil() as u64
}

pub fn update_bullet_position(
    bullet: &mut Bullet,
    maze: &Maze,
    current_tick: u64,
) -> BulletUpdateResult {
    let mut result = BulletUpdateResult::default();

    bullet.advance(1);

    if bullet.is_expired(current_tick) || bullet.has_bounced_enough() {
        result.should_remove = true;
        result.event_type = BulletEventType::Expire;
    } else {
        if bounce_off_ground(
            &mut bullet.position,
            &mut bullet.velocity,
            &mut bullet.bounces,
        ) {
            result.hit_inanimate = true;
        }

        match bounce_off_wall(
            &mut bullet.position,
            &mut bullet.velocity,
            &mut bullet.bounces,
            maze,
        ) {
            WallBounce::Bounce => {
                result.hit_inanimate = true;
            }
            WallBounce::Stuck => {
                result.should_remove = true;
                result.event_type = BulletEventType::Expire;
            }
            WallBounce::None => {}
        }
    }

    result
}

#[derive(Debug, Default)]
pub struct BulletUpdateResult {
    pub should_remove: bool,
    pub hit_inanimate: bool,
    pub event_type: BulletEventType,
}

#[derive(Debug, Default, PartialEq)]
pub enum BulletEventType {
    #[default]
    None,
    Expire,
}

pub fn check_player_collision(
    bullet: &mut Bullet,
    player_position: Vec3,
    player_health: u8,
) -> PlayerCollisionResult {
    if !is_bullet_colliding_with_player(bullet.position, player_position) {
        return PlayerCollisionResult::default();
    }

    let new_health = player_health.saturating_sub(1);

    if new_health > 0 {
        let delta = bullet.position - player_position;
        let distance = delta.length();

        if distance > 0.001 {
            let bullet_direction = bullet.velocity.normalize_or_zero();
            let collision_radius = player::RADIUS + BULLET_CORE_RADIUS;

            // Calculate intersection between the ray and the `collision_sphere`
            // intersection to find entry point.
            let a = bullet_direction.dot(bullet_direction);
            let b = 2.0 * bullet_direction.dot(player_position - bullet.position);
            let c = (player_position - bullet.position).length_squared()
                - collision_radius * collision_radius;

            let discriminant = b * b - 4.0 * a * c;

            if discriminant >= 0.0 {
                let t1 = (-b - discriminant.sqrt()) / (2.0 * a);
                let t2 = (-b + discriminant.sqrt()) / (2.0 * a);

                // Use the entry point (most negative t).
                let entry_t = t1.min(t2);

                let entry_point = bullet.position + bullet_direction * entry_t;
                let normal = (entry_point - player_position).normalize();

                // Move the bullet to the entry point and apply bounce.
                bullet.position = entry_point;
                bullet.redirect(normal);
            } else {
                // Fallback: intesection calculation failed.
                let normal = delta.normalize();
                bullet.redirect(normal);
            }
        } else {
            let normal = bullet.velocity.normalize_or_zero();
            bullet.position = player_position + normal * (player::RADIUS + BULLET_CORE_RADIUS);
            bullet.redirect(normal);
        }

        PlayerCollisionResult {
            hit_player: true,
            new_health,
            should_remove_bullet: false,
        }
    } else {
        PlayerCollisionResult {
            hit_player: true,
            new_health,
            should_remove_bullet: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct PlayerCollisionResult {
    pub hit_player: bool,
    pub new_health: u8,
    pub should_remove_bullet: bool,
}

fn redirect(velocity: &mut Vec3, bounces: &mut u8, normal: Vec3) {
    *velocity = reflect(*velocity, normal);
    *bounces += 1;
}

fn reflect(direction: Vec3, normal: Vec3) -> Vec3 {
    direction - 2.0 * direction.dot(normal) * normal
}
