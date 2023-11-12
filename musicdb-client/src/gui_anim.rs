use std::{
    ops::{Add, AddAssign, Mul, MulAssign, Sub},
    time::{Duration, Instant},
};

pub struct AnimationController<F> {
    pub last_updated: Instant,
    pub value: F,
    pub speed: F,
    pub max_speed: F,
    pub target: F,
    /// while the time remaining to finish the animation is above this, we accelerate (higher -> stop acceleration earlier)
    pub accel_until: F,
    /// if the time remaining to finish the animation drops below this, we decelerate (higher -> start decelerating earlier)
    pub decel_while: F,
    pub acceleration: F,
}

pub trait Float:
    Sized
    + Clone
    + Copy
    + Add<Self, Output = Self>
    + Sub<Self, Output = Self>
    + std::ops::Neg<Output = Self>
    + Mul<Self, Output = Self>
    + MulAssign<Self>
    + AddAssign<Self>
    + PartialOrd<Self>
{
    fn zero() -> Self;
    /// 1/1000
    fn milli() -> Self;
    fn duration_secs(d: Duration) -> Self;
    fn abs(self) -> Self {
        if self < Self::zero() {
            -self
        } else {
            self
        }
    }
}

impl<F: Float> AnimationController<F> {
    pub fn new(
        value: F,
        target: F,
        acceleration: F,
        max_speed: F,
        accel_until: F,
        decel_while: F,
        now: Instant,
    ) -> Self {
        AnimationController {
            last_updated: now,
            value,
            speed: F::zero(),
            max_speed,
            target,
            accel_until,
            decel_while,
            acceleration,
        }
    }
    pub fn ignore_elapsed_time(&mut self, now: Instant) {
        self.last_updated = now;
    }
    pub fn update(&mut self, now: Instant, instant: bool) -> bool {
        let changed = if self.target != self.value {
            if instant {
                self.value = self.target;
            } else {
                let inc = self.target > self.value;
                let seconds = F::duration_secs(now.duration_since(self.last_updated));
                let ref1 = self.value + self.speed * self.accel_until;
                let ref2 = self.value + self.speed * self.decel_while;
                let speed_diff = match (ref1 < self.target, ref2 > self.target) {
                    (true, false) => self.acceleration,
                    (false, true) => -self.acceleration,
                    (true, true) | (false, false) => F::zero(),
                };
                self.speed += speed_diff;
                if self.speed.abs() > self.max_speed {
                    if self.speed < F::zero() {
                        self.speed = -self.max_speed;
                    } else {
                        self.speed = self.max_speed;
                    }
                }
                self.value += self.speed * seconds;
                self.speed += speed_diff;
                if (self.target - self.value).abs() < self.speed * F::milli()
                    || inc != (self.target > self.value)
                {
                    // overshoot or target reached
                    self.value = self.target;
                    self.speed = F::zero();
                }
            }
            true
        } else {
            false
        };
        self.last_updated = now;
        changed
    }
}

impl Float for f32 {
    fn zero() -> Self {
        0.0
    }
    fn milli() -> Self {
        0.001
    }
    fn duration_secs(d: Duration) -> Self {
        d.as_secs_f32().min(0.1)
    }
}
impl Float for f64 {
    fn zero() -> Self {
        0.0
    }
    fn milli() -> Self {
        0.001
    }
    fn duration_secs(d: Duration) -> Self {
        d.as_secs_f64().min(0.1)
    }
}
