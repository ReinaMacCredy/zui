//! The time source animations read, which a test driver can pause and step.

use scheduler::Instant;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

static ENGAGED: AtomicBool = AtomicBool::new(false);
static CLOCK: Mutex<Clock> = Mutex::new(Clock {
    frozen: None,
    base: None,
    advanced: Duration::ZERO,
});

struct Clock {
    frozen: Option<Instant>,
    base: Option<(Instant, Instant)>,
    advanced: Duration,
}

impl Clock {
    fn now(&self) -> Instant {
        if let Some(frozen) = self.frozen {
            return frozen;
        }
        let real = Instant::now();
        match self.base {
            Some((real_at, virtual_at)) => virtual_at + real.saturating_duration_since(real_at),
            None => real,
        }
    }
}

fn clock() -> MutexGuard<'static, Clock> {
    CLOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The current animation instant: wall time until [`pause`] is first called.
pub fn now() -> Instant {
    if !ENGAGED.load(Ordering::Relaxed) {
        return Instant::now();
    }
    clock().now()
}

/// Animation time elapsed since `start`, read from [`now`].
pub fn since(start: Instant) -> Duration {
    now().saturating_duration_since(start)
}

/// Freeze animation time at the current instant. Pausing twice keeps the first instant.
pub fn pause() {
    let mut clock = clock();
    if clock.frozen.is_none() {
        clock.frozen = Some(clock.now());
        clock.advanced = Duration::ZERO;
    }
    ENGAGED.store(true, Ordering::Relaxed);
}

/// Move paused animation time forward by `by`, returning the time advanced since the pause.
/// Returns `None` when the clock is not paused.
pub fn advance(by: Duration) -> Option<Duration> {
    let mut clock = clock();
    let frozen = clock.frozen.as_mut()?;
    *frozen += by;
    clock.advanced += by;
    Some(clock.advanced)
}

/// Let animation time run again from the paused instant, without a jump.
pub fn resume() {
    let mut clock = clock();
    if let Some(frozen) = clock.frozen.take() {
        clock.base = Some((Instant::now(), frozen));
    }
}

/// The time advanced since the pause while paused, `None` while running.
pub fn paused() -> Option<Duration> {
    if !ENGAGED.load(Ordering::Relaxed) {
        return None;
    }
    let clock = clock();
    clock.frozen.map(|_| clock.advanced)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_freezes_advance_steps_and_resume_continues_without_a_jump() {
        pause();
        let frozen = now();
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(now(), frozen);
        assert_eq!(
            advance(Duration::from_millis(50)),
            Some(Duration::from_millis(50))
        );
        assert_eq!(paused(), Some(Duration::from_millis(50)));
        assert_eq!(since(frozen), Duration::from_millis(50));
        resume();
        assert_eq!(paused(), None);
        assert_eq!(advance(Duration::from_millis(5)), None);
        std::thread::sleep(Duration::from_millis(20));
        let resumed = since(frozen);
        assert!(
            resumed >= Duration::from_millis(70) && resumed < Duration::from_millis(500),
            "{resumed:?}"
        );
    }
}
