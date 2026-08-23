//! Particles, screen shake, and flash.
//!
//! All decay is by wall-clock time, never per frame, so the effects look the
//! same at 30fps and 144Hz.

use crate::game::rng::Pcg32;
use crate::render::canvas::Canvas;
use crate::render::color::Rgb;

/// Fixed regardless of body length, so a long snake does not cost more to kill.
pub const DEATH_PARTICLES: usize = 400;
pub const EAT_PARTICLES: usize = 26;

const SHAKE_TAU: f32 = 0.10;
const FLASH_TAU: f32 = 0.13;
const GRAVITY: f32 = 26.0;
const EAT_SHAKE: f32 = 0.8;
const DEATH_SHAKE: f32 = 3.2;

#[derive(Clone, Copy)]
struct Particle {
    pos: (f32, f32),
    vel: (f32, f32),
    life: f32,
    max_life: f32,
    color: Rgb,
    drag: f32,
}

pub struct Fx {
    particles: Vec<Particle>,
    rng: Pcg32,
    shake: f32,
    flash: f32,
    clock: f32,
}

impl Fx {
    pub fn new(seed: u64) -> Self {
        Fx {
            particles: Vec::with_capacity(DEATH_PARTICLES + 64),
            rng: Pcg32::new(seed),
            shake: 0.0,
            flash: 0.0,
            clock: 0.0,
        }
    }

    #[allow(dead_code)] // asserted by the effects tests and the benchmark
    pub fn live(&self) -> usize {
        self.particles.len()
    }

    pub fn flash(&self) -> f32 {
        self.flash
    }

    fn unit(&mut self) -> f32 {
        self.rng.next_u32() as f32 / u32::MAX as f32
    }

    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    pub fn clear(&mut self) {
        self.particles.clear();
        self.shake = 0.0;
        self.flash = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        self.shake *= (-dt / SHAKE_TAU).exp();
        self.flash *= (-dt / FLASH_TAU).exp();
        if self.shake < 1e-3 {
            self.shake = 0.0;
        }
        if self.flash < 1e-4 {
            self.flash = 0.0;
        }

        for p in self.particles.iter_mut() {
            p.life -= dt;
            let k = (-dt * p.drag).exp();
            p.vel.0 *= k;
            p.vel.1 = p.vel.1 * k + GRAVITY * dt;
            p.pos.0 += p.vel.0 * dt;
            p.pos.1 += p.vel.1 * dt;
        }
        self.particles.retain(|p| p.life > 0.0);
    }

    /// A directional spray, thrown back against the direction of travel.
    pub fn emit_eat(&mut self, at: (f32, f32), dir: (f32, f32), color: Rgb) {
        for _ in 0..EAT_PARTICLES {
            let spread = self.signed() * 0.9;
            let speed = 14.0 + self.unit() * 34.0;
            let vx = -dir.0 * speed + spread * speed * 0.7;
            let vy = -dir.1 * speed + spread * speed * 0.7;
            let life = 0.22 + self.unit() * 0.30;
            self.particles.push(Particle {
                pos: at,
                vel: (vx, vy),
                life,
                max_life: life,
                color,
                drag: 3.0,
            });
        }
        self.shake = self.shake.max(EAT_SHAKE);
    }

    /// The body dissolves: particles seeded along the whole ribbon, so the
    /// snake visibly comes apart rather than exploding from one point.
    pub fn emit_death(&mut self, path: &[(f32, f32)], color: Rgb) {
        if path.is_empty() {
            return;
        }
        for i in 0..DEATH_PARTICLES {
            let at = path[i % path.len()];
            let angle = self.unit() * std::f32::consts::TAU;
            let speed = 8.0 + self.unit() * 64.0;
            let life = 0.35 + self.unit() * 0.85;
            self.particles.push(Particle {
                pos: at,
                vel: (angle.cos() * speed, angle.sin() * speed),
                life,
                max_life: life,
                color,
                drag: 1.6,
            });
        }
        self.shake = DEATH_SHAKE;
        self.flash = 1.0;
    }

    /// Decaying offset applied to the canvas sample origin.
    pub fn shake_offset(&self) -> (f32, f32) {
        if self.shake <= 0.0 {
            return (0.0, 0.0);
        }
        let a = self.clock * 47.0;
        let b = self.clock * 61.0;
        (self.shake * a.sin(), self.shake * b.cos() * 0.6)
    }

    pub fn draw(&self, c: &mut Canvas) {
        for p in &self.particles {
            let k = (p.life / p.max_life).clamp(0.0, 1.0);
            let fade = k * k;
            let col = [p.color[0] * fade, p.color[1] * fade, p.color[2] * fade];
            c.blend(
                p.pos.0.round() as i32,
                p.pos.1.round() as i32,
                col,
                fade.min(1.0),
            );
            c.add_glow(p.pos.0, p.pos.1, [col[0] * 0.5, col[1] * 0.5, col[2] * 0.5]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_eat_burst_spawns_particles_and_a_small_shake() {
        let mut fx = Fx::new(1);
        assert_eq!(fx.live(), 0);
        fx.emit_eat((10.0, 10.0), (1.0, 0.0), [1.0, 0.5, 0.5]);
        assert_eq!(fx.live(), EAT_PARTICLES);
        fx.update(0.016);
        let (sx, sy) = fx.shake_offset();
        assert!(sx.abs() + sy.abs() > 0.0, "an eat should shake a little");
    }

    #[test]
    fn a_death_burst_is_a_fixed_size_regardless_of_body_length() {
        let short: Vec<(f32, f32)> = (0..4).map(|i| (i as f32, 0.0)).collect();
        let long: Vec<(f32, f32)> = (0..120).map(|i| (i as f32, 0.0)).collect();
        let mut a = Fx::new(1);
        a.emit_death(&short, [1.0; 3]);
        let mut b = Fx::new(1);
        b.emit_death(&long, [1.0; 3]);
        assert_eq!(a.live(), DEATH_PARTICLES);
        assert_eq!(b.live(), DEATH_PARTICLES);
    }

    #[test]
    fn a_death_burst_shakes_harder_than_an_eat() {
        let mut a = Fx::new(1);
        a.emit_eat((10.0, 10.0), (1.0, 0.0), [1.0; 3]);
        let eat = a.shake;
        let mut b = Fx::new(1);
        b.emit_death(&[(10.0, 10.0)], [1.0; 3]);
        assert!(b.shake > eat, "death {} vs eat {eat}", b.shake);
    }

    #[test]
    fn particles_expire() {
        let mut fx = Fx::new(2);
        fx.emit_eat((10.0, 10.0), (1.0, 0.0), [1.0; 3]);
        for _ in 0..600 {
            fx.update(0.016);
        }
        assert_eq!(fx.live(), 0);
    }

    #[test]
    fn shake_and_flash_decay_to_nothing() {
        let mut fx = Fx::new(3);
        fx.emit_death(&[(5.0, 5.0)], [1.0; 3]);
        assert!(fx.flash() > 0.0);
        for _ in 0..200 {
            fx.update(0.016);
        }
        assert_eq!(fx.flash(), 0.0);
        assert_eq!(fx.shake_offset(), (0.0, 0.0));
    }

    #[test]
    fn decay_is_frame_rate_independent() {
        let mut a = Fx::new(4);
        a.emit_death(&[(5.0, 5.0)], [1.0; 3]);
        a.update(0.1);
        let mut b = Fx::new(4);
        b.emit_death(&[(5.0, 5.0)], [1.0; 3]);
        for _ in 0..10 {
            b.update(0.01);
        }
        assert!(
            (a.flash() - b.flash()).abs() < 1e-3,
            "{} vs {}",
            a.flash(),
            b.flash()
        );
    }

    #[test]
    fn particles_put_light_on_the_canvas() {
        let mut fx = Fx::new(5);
        fx.emit_eat((16.0, 16.0), (1.0, 0.0), [1.0, 1.0, 1.0]);
        fx.update(0.016);
        let mut c = Canvas::new(32, 32);
        c.clear_base([0.0; 3]);
        fx.draw(&mut c);
        c.blur_glow();
        let total: f32 = (0..32)
            .flat_map(|y| (0..32).map(move |x| (x, y)))
            .map(|(x, y)| c.sample(x, y).iter().sum::<f32>())
            .sum();
        assert!(total > 0.0, "particles put no light on the canvas");
    }

    #[test]
    fn the_same_seed_produces_the_same_burst() {
        let burst = |seed| {
            let mut fx = Fx::new(seed);
            fx.emit_eat((10.0, 10.0), (0.0, -1.0), [1.0; 3]);
            for _ in 0..5 {
                fx.update(0.016);
            }
            (fx.live(), fx.shake_offset())
        };
        assert_eq!(burst(9), burst(9));
    }

    #[test]
    fn an_eat_burst_is_thrown_back_against_the_direction_of_travel() {
        let mut fx = Fx::new(6);
        fx.emit_eat((100.0, 100.0), (1.0, 0.0), [1.0; 3]);
        let behind = fx.particles.iter().filter(|p| p.vel.0 < 0.0).count();
        assert!(
            behind > EAT_PARTICLES / 2,
            "{behind} of {EAT_PARTICLES} went backwards"
        );
    }

    #[test]
    fn clearing_removes_everything() {
        let mut fx = Fx::new(7);
        fx.emit_death(&[(1.0, 1.0)], [1.0; 3]);
        fx.clear();
        assert_eq!(fx.live(), 0);
        assert_eq!(fx.flash(), 0.0);
        assert_eq!(fx.shake_offset(), (0.0, 0.0));
    }

    #[test]
    fn an_empty_path_emits_nothing_rather_than_panicking() {
        let mut fx = Fx::new(8);
        fx.emit_death(&[], [1.0; 3]);
        assert_eq!(fx.live(), 0);
    }

    #[test]
    fn drawing_particles_that_drift_off_canvas_does_not_panic() {
        let mut fx = Fx::new(9);
        fx.emit_death(&[(2.0, 2.0)], [1.0; 3]);
        let mut c = Canvas::new(8, 8);
        for _ in 0..120 {
            fx.update(0.016);
            c.clear_base([0.0; 3]);
            fx.draw(&mut c);
        }
    }
}
