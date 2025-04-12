use std::time::{ Duration, Instant };

pub struct PerformanceMetrics {
    pub request_count: u64,
    pub total_time: Duration,
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        PerformanceMetrics {
            request_count: 0,
            total_time: Duration::new(0, 0),
        }
    }

    pub fn record_request(&mut self, duration: Duration) {
        self.request_count += 1;
        self.total_time += duration;
    }

    pub fn average_time(&self) -> Option<Duration> {
        if self.request_count == 0 {
            None
        } else {
            Some(self.total_time / (self.request_count as u32))
        }
    }
}

pub fn measure_request<F>(f: F) -> Duration where F: FnOnce() -> () {
    let start = Instant::now();
    f();
    start.elapsed()
}
