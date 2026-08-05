//! Time adapters implementing the application `Clock` port.

use std::time::SystemTime;

use cantina_application::ports::Clock;

/// Wall-clock adapter backed by `SystemTime`.
pub struct SystemClock;

impl Default for SystemClock {
    fn default() -> Self {
        Self
    }
}

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[cfg(test)]
mod tests {
    use super::SystemClock;
    use cantina_application::ports::Clock;

    #[test]
    fn system_clock_reports_a_time() {
        let clock = SystemClock;
        let _ = clock.now();
    }
}
