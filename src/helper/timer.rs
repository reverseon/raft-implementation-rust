use std::time::Duration;

use rand::Rng;

pub struct RandomizedTimer {
    pub start_time: std::time::Instant,
    _duration: std::time::Duration,
    pub lower_ceiling_ms: u64,
    pub upper_ceiling_ms: u64,
}

impl RandomizedTimer {
    pub fn new(lower_ceiling_ms: u64, upper_ceiling_ms: u64) -> RandomizedTimer {
        let mut rng = rand::thread_rng();
        let timemillis = rng.gen_range(lower_ceiling_ms..upper_ceiling_ms);
        let duration = std::time::Duration::from_millis(timemillis);
        let start_time = std::time::Instant::now();
        return RandomizedTimer {
            start_time,
            _duration: duration,
            lower_ceiling_ms,
            upper_ceiling_ms,
        };
    }

    pub fn is_expired(&self) -> bool {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.start_time);
        if elapsed.as_millis() > self._duration.as_millis() {
            return true;
        } else {
            return false;
        }
    }

    pub fn reset(&mut self) {
        let mut rng = rand::thread_rng();
        let timemillis = rng.gen_range(self.lower_ceiling_ms..self.upper_ceiling_ms);
        let duration = std::time::Duration::from_millis(timemillis);
        self.start_time = std::time::Instant::now();
        self._duration = duration;
    }
    pub fn get_duration(&self) -> Duration {
        return self._duration;
    } 
}

pub fn get_randomized_duration(lower_ceiling_ms: u64, upper_ceiling_ms: u64) -> Duration {
    let mut rng = rand::thread_rng();
    let timemillis = rng.gen_range(lower_ceiling_ms..upper_ceiling_ms);
    let duration = std::time::Duration::from_millis(timemillis);
    return duration;
}