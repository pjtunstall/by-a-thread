use glam::{Vec3, vec3};

use crate::{
    constants::{TICK_SECS, TICK_SECS_F32},
    maze::{CELL_SIZE, Maze},
    player,
};

pub const MAX_BULLETS_PER_PLAYER: usize = 24;
pub const BULLET_RADIUS: f32 = 0.1;
pub const FIRE_COOLDOWN_SECS: f64 = 0.1;
pub const SPEED: f32 = 720.0;
pub const LIFESPAN_SECS: f64 = 2.5;
pub const MAX_BOUNCES: u8 = 5;
pub const BULLET_SPAWN_OFFSET: f32 = player::RADIUS + BULLET_RADIUS + 0.1;

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

// pub fn bounce_off_ground(position: &mut Vec3, velocity: &mut Vec3, bounces: &mut u8) -> bool {
//     if position.y < BULLET_RADIUS {
//         position.y = BULLET_RADIUS;
//         if velocity.y < 0.0 {
//             velocity.y *= -1.0;
//             *bounces += 1;
//             return true;
//         }
//     }

//     false
// }

pub fn bounce_off_ground(position: &mut Vec3, velocity: &mut Vec3, bounces: &mut u8) -> bool {
    if position.y > BULLET_RADIUS || velocity.y >= 0.0 {
        return false;
    }

    // Time to impact: negative value, represents time in the past. Due to the
    // early returns, we can be sure that `t` <= 0.0.
    let t = (BULLET_RADIUS - position.y) / velocity.y;

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
    let bullet_is_not_above_wall_height = position.y < CELL_SIZE;
    let is_bullet_colliding_with_a_wall =
        bullet_is_not_above_wall_height && !maze.is_sphere_clear(position, BULLET_RADIUS);

    if !is_bullet_colliding_with_a_wall {
        return WallBounce::None;
    }

    let direction = velocity.normalize_or_zero();
    if direction == Vec3::ZERO {
        return WallBounce::Stuck;
    }

    let speed = velocity.length() * TICK_SECS_F32;
    let normal = maze.get_wall_normal(*position, direction, speed);
    if normal == Vec3::ZERO {
        WallBounce::Stuck
    } else {
        redirect(velocity, bounces, normal);
        WallBounce::Bounce
    }
}

pub fn is_bullet_colliding_with_player(bullet_position: Vec3, player_position: Vec3) -> bool {
    bullet_position.distance(player_position) < BULLET_RADIUS + player::RADIUS
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
            let collision_radius = player::RADIUS + BULLET_RADIUS;

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
            bullet.position = player_position + normal * (player::RADIUS + BULLET_RADIUS);
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
