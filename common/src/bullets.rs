use macroquad::{
    audio::{PlaySoundParams, Sound, play_sound},
    prelude::*,
};

use super::{
    fade::{self, Fade},
    maze::{CELL_SIZE, Maze},
    remote_player::RemotePlayer,
    session::PLAYER_RADIUS,
};

pub const MAX_BULLETS: usize = 24;
pub const RADIUS: f32 = 0.1;
pub const FIRE_COOLDOWN: f64 = 0.1;
const SPEED: f32 = 12.0;
const LIFESPAN: f32 = 2.5;
const MAX_BOUNCES: u8 = 5;

#[derive(Debug, Clone, Copy)]
struct Bullet {
    position: Vec3,
    direction: Vec3,
    birthday: f64,
    bounces: u8,
}

impl Bullet {
    fn new(position: Vec3, direction: Vec3) -> Self {
        Self {
            position,
            direction,
            birthday: miniquad::date::now(),
            bounces: 0,
        }
    }

    fn update_position(&mut self) {
        self.position.x += SPEED * self.direction.x;
        self.position.y += SPEED * self.direction.y;
        self.position.z += SPEED * self.direction.z;
    }

    fn is_expired(&self) -> bool {
        let now = miniquad::date::now();
        let elapsed = (now - self.birthday) as f32;
        elapsed > LIFESPAN
    }

    fn has_bounced_enough(&self) -> bool {
        self.bounces > MAX_BOUNCES
    }

    fn bounce_off_ground(&mut self) -> bool {
        if self.position.y < 0.0 && self.direction.y < 0.0 {
            self.direction.y *= -1.0;
            self.bounces += 1;
            true
        } else {
            false
        }
    }

    fn redirect(&mut self, normal: Vec3) {
        self.direction = reflect(self.direction, normal);
        self.bounces += 1;
    }

    fn fade_amount(&self) -> f32 {
        let now = miniquad::date::now();
        let elapsed = (now - self.birthday) as f32;
        1.0 - (elapsed / LIFESPAN).min(1.0)
    }
}

pub struct Bullets {
    active_bullets: usize,
    bullets: [Bullet; MAX_BULLETS],
    gun_sound: Sound,
    remote_player_hit_sound: Sound,
    local_player_hit_sound: Sound,
    remote_player_killed_sound: Sound,
    local_player_killed_sound: Sound,
}

impl Bullets {
    pub fn new(
        gun_sound: Sound,
        clang: Sound,
        deep_clang: Sound,
        shatter_sound: Sound,
        bell_sound: Sound,
    ) -> Self {
        debug_assert!(MAX_BULLETS < 32);
        Bullets {
            active_bullets: 0,
            bullets: [Bullet::new(Vec3::ZERO, Vec3::ZERO); MAX_BULLETS],
            gun_sound,
            remote_player_hit_sound: clang,
            local_player_hit_sound: deep_clang,
            remote_player_killed_sound: shatter_sound,
            local_player_killed_sound: bell_sound,
        }
    }

    pub fn fire(&mut self, position: Vec3, direction: Vec3) {
        for i in 0..MAX_BULLETS {
            if !self.is_active(i) {
                self.active_bullets |= 1 << i;
                self.bullets[i] = Bullet::new(position, direction);

                play_sound(
                    &self.gun_sound,
                    PlaySoundParams {
                        looped: false,
                        volume: 1.0,
                    },
                );
                break;
            }
        }
    }

    pub fn is_maxed_out(&self) -> bool {
        self.active_bullets > (1 << MAX_BULLETS) - 1
    }

    fn is_active(&self, i: usize) -> bool {
        self.active_bullets & (1 << i) != 0
    }

    fn remove(&mut self, i: usize) {
        self.active_bullets &= !(1 << i);
    }

    fn draw_bullet(&self, i: usize) {
        let bullet = &self.bullets[i];
        let fade = bullet.fade_amount();
        draw_sphere(
            bullet.position,
            4.0,
            None,
            Color::new(1.00, fade, fade, fade),
        );
    }

    pub fn update(
        &mut self,
        maze: &Maze,
        flash: &mut Option<Fade>,
        fade_to_black: &mut Option<Fade>,
        local_player_health: &mut u8,
        local_player_position: &Vec3,
        local_player_is_alive: &mut bool,
        remote_players: &mut Vec<RemotePlayer>,
    ) {
        for i in 0..MAX_BULLETS {
            if self.is_active(i) {
                self.update_bullet(
                    i,
                    maze,
                    flash,
                    fade_to_black,
                    local_player_health,
                    local_player_position,
                    local_player_is_alive,
                    remote_players,
                );
                self.draw_bullet(i);
            }
        }
    }

    fn update_bullet(
        &mut self,
        i: usize,
        maze: &Maze,
        flash: &mut Option<Fade>,
        fade_to_black: &mut Option<Fade>,
        local_player_health: &mut u8,
        local_player_position: &Vec3,
        local_player_is_alive: &mut bool,
        remote_players: &mut Vec<RemotePlayer>,
    ) {
        self.bullets[i].update_position();

        if self.bullets[i].is_expired() || self.bullets[i].has_bounced_enough() {
            self.remove(i);
        } else {
            self.bullets[i].bounce_off_ground();
            self.bounce_off_wall(i, maze);
            self.hit_local_player(
                i,
                local_player_health,
                local_player_position,
                fade_to_black,
                flash,
                local_player_is_alive,
            );
            self.hit_remote_player(i, remote_players);
        }
    }

    fn bounce_off_wall(&mut self, i: usize, maze: &Maze) -> bool {
        let bullet = &mut self.bullets[i];
        let bullet_is_not_above_wall_height = bullet.position.y < CELL_SIZE;
        let has_bullet_crossed_a_wall = !maze.is_way_clear(&bullet.position);
        let is_bullet_colliding_with_a_wall =
            bullet_is_not_above_wall_height && has_bullet_crossed_a_wall;

        if is_bullet_colliding_with_a_wall {
            let normal = maze.get_wall_normal(bullet.position, bullet.direction, SPEED);
            if normal == Vec3::ZERO {
                self.remove(i);
            } else {
                bullet.redirect(normal);
            }
            true
        } else {
            false
        }
    }

    fn hit_local_player(
        &mut self,
        i: usize,
        local_player_health: &mut u8,
        local_player_position: &Vec3,
        fade_to_black: &mut Option<Fade>,
        flash: &mut Option<Fade>,
        local_player_is_alive: &mut bool,
    ) -> bool {
        let bullet = &mut self.bullets[i];
        if bullet.position.distance(*local_player_position) < RADIUS + PLAYER_RADIUS {
            *flash = Some(fade::new_flash());
            if *local_player_health > 0 {
                *local_player_health -= 1;
                if *local_player_health == 0 {
                    play_sound(
                        &self.local_player_killed_sound,
                        PlaySoundParams {
                            looped: false,
                            volume: 1.0,
                        },
                    );
                    *fade_to_black = Some(fade::new_fade_to_black());
                    *local_player_is_alive = false;
                } else {
                    play_sound(
                        &self.local_player_hit_sound,
                        PlaySoundParams {
                            looped: false,
                            volume: 1.0,
                        },
                    );
                    let normal = (*local_player_position - bullet.position).normalize();
                    bullet.redirect(normal);
                }
            }
            self.remove(i);
            true
        } else {
            false
        }
    }

    fn hit_remote_player(&mut self, i: usize, remote_players: &mut Vec<RemotePlayer>) -> bool {
        let bullet = &mut self.bullets[i];
        for remote_player in remote_players {
            if remote_player.is_alive {
                let colliding =
                    bullet.position.distance(remote_player.position) < RADIUS + PLAYER_RADIUS;
                if colliding {
                    if remote_player.health > 1 {
                        play_sound(
                            &self.remote_player_hit_sound,
                            PlaySoundParams {
                                looped: false,
                                volume: 1.0,
                            },
                        );
                        remote_player.health -= 1;
                        remote_player.rotation_speed += 0.002;
                        let normal = (remote_player.position - bullet.position).normalize();
                        bullet.redirect(normal);
                    } else {
                        play_sound(
                            &self.remote_player_killed_sound,
                            PlaySoundParams {
                                looped: false,
                                volume: 1.0,
                            },
                        );
                        self.remove(i);
                        remote_player.is_alive = false;
                    }
                    return true;
                }
            }
        }

        false
    }
}

fn reflect(direction: Vec3, normal: Vec3) -> Vec3 {
    direction - 2.0 * direction.dot(normal) * normal
}
