use std::time::Instant;

use uianimator::{default_animator_f64_quadratic::DefaultAnimatorF64Quadratic, Animator};

pub struct AnimationController {
    speed: f64,
    anim: DefaultAnimatorF64Quadratic,
}

impl AnimationController {
    pub fn new(value: f64, target: f64, speed: f64) -> Self {
        let mut anim = DefaultAnimatorF64Quadratic::new(value, speed);
        if value != target {
            anim.set_target(target, Instant::now());
        }
        AnimationController { speed, anim }
    }
    pub fn target(&self) -> f64 {
        self.anim.target()
    }
    pub fn set_target(&mut self, now: Instant, target: f64) {
        self.anim.set_target(target, now);
    }
    pub fn update(&mut self, now: Instant, instant: bool) -> Result<f64, f64> {
        if self.anim.target() != self.anim.get_value(now) {
            if instant {
                let target = self.anim.target();
                self.anim = DefaultAnimatorF64Quadratic::new(target, self.speed);
                Ok(target)
            } else {
                Ok(self.anim.get_value(now))
            }
        } else {
            Err(self.anim.target())
        }
    }
    pub fn value(&mut self, now: Instant) -> f64 {
        match self.update(now, false) {
            Ok(v) | Err(v) => v,
        }
    }
}
