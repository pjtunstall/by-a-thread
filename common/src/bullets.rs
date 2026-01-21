use glam::{Vec3, vec3};

use crate::{
    constants::{TICK_SECS, TICK_SECS_F32},
    maze::{CELL_SIZE, Maze},
    player,
};

pub const MAX_BULLETS_PER_PLAYER: usize = 24;
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

pub fn bounce_off_wall(
    position: &Vec3,
    velocity: &mut Vec3,
    bounces: &mut u8,
    maze: &Maze,
) -> WallBounce {
    let bullet_is_not_above_wall_height = position.y - BULLET_SHELL_RADIUS < CELL_SIZE;
    if !bullet_is_not_above_wall_height {
        return WallBounce::None;
    }

    let direction = velocity.normalize_or_zero();
    if direction == Vec3::ZERO {
        return WallBounce::Stuck;
    }

    // Trace back along bullet path to find wall intersection.
    let trace_distance = velocity.length() * TICK_SECS_F32;
    let trace_direction = -direction;
    let grid = &maze.grid;
    let grid_width = grid[0].len() as isize;
    let grid_height = grid.len() as isize;

    // Adjust start position to account for shell radius.
    let start_pos = *position - direction * BULLET_SHELL_RADIUS;

    const TRACE_STEP_SIZE: f32 = 2.0;
    let num_steps = (trace_distance / TRACE_STEP_SIZE).ceil() as usize;

    let mut previous_cell_type: Option<u8> = None;

    for step in 0..=num_steps {
        let trace_pos = start_pos + trace_direction * (step as f32 * TRACE_STEP_SIZE);
        let grid_x = (trace_pos.x / CELL_SIZE).floor() as isize;
        let grid_z = (trace_pos.z / CELL_SIZE).floor() as isize;

        // Check if we're outside the maze bounds.
        if grid_x < 0 || grid_z < 0 || grid_x >= grid_width || grid_z >= grid_height {
            continue;
        }

        let current_cell_type = grid[grid_z as usize][grid_x as usize] as u8;

        if let Some(prev_type) = previous_cell_type {
            // Found transition between cell types.
            if prev_type != current_cell_type {
                // Determine which face was crossed based on position.
                let cell_min_x = grid_x as f32 * CELL_SIZE;
                let cell_max_x = cell_min_x + CELL_SIZE;
                let cell_min_z = grid_z as f32 * CELL_SIZE;
                let cell_max_z = cell_min_z + CELL_SIZE;

                let normal = if trace_pos.x < cell_min_x + 0.1 {
                    Vec3::new(-1.0, 0.0, 0.0)
                } else if trace_pos.x > cell_max_x - 0.1 {
                    Vec3::new(1.0, 0.0, 0.0)
                } else if trace_pos.z < cell_min_z + 0.1 {
                    Vec3::new(0.0, 0.0, -1.0)
                } else if trace_pos.z > cell_max_z - 0.1 {
                    Vec3::new(0.0, 0.0, 1.0)
                } else {
                    // Fallback: use opposite of direction.
                    -trace_direction
                };

                // Move bullet to intersection point (accounting for shell radius) and bounce.
                redirect(velocity, bounces, normal);
                return WallBounce::Bounce;
            }
        }

        previous_cell_type = Some(current_cell_type);
    }

    WallBounce::None
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
            &bullet.position,
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
