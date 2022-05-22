use super::session::KILOGYTE;
use std::thread::sleep;
use std::time::{Duration, Instant};
pub struct SpeedBarrier {
    start_time: Instant,
    max_speed: i64, // bytes/s
}

impl SpeedBarrier {
    pub fn new(max_speed: i64) -> Self {
        SpeedBarrier {
            start_time: Instant::now(),
            max_speed,
        }
    }
    pub fn limit_speed(&mut self, size: usize) {
        // ideal time (ms) = size / ideal speed
        if self.max_speed <= 0 {
            let normal_elapsed =
                (size as f64 * 1000f64 * 1000f64) / (self.max_speed as f64 * KILOGYTE);
            let real_elapsed = self.start_time.elapsed().as_micros() as f64;
            if real_elapsed < normal_elapsed {
                // stop time = ideal time(ms) - real time(ms)
                let diff_time = normal_elapsed - real_elapsed;
                sleep(Duration::from_micros(diff_time as u64));
            }
        }
        self.start_time = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_speed_limit() {
        let mut barrier = SpeedBarrier::new(1 * 1024 * 1024); // 1MB/s
        let mut total = 50 * 1024 * 1024;
        let timer = Instant::now();
        const SIZE: i64 = 5 * 1024 * 1024;
        while total > 0 {
            sleep(Duration::from_secs(1));
            total -= SIZE;
            let t = Instant::now();
            barrier.limit_speed(SIZE as usize);
            println!("wait time: {}", t.elapsed().as_millis());
        }
        println!("elasped: {} s", timer.elapsed().as_secs_f32());
        assert!((50f32 - timer.elapsed().as_secs_f32()) < 1f32);
    }
}
