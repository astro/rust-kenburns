use util::*;

pub struct FrameCounter {
    ticks: u32,
    last_reset: u64,
    interval: u64
}

impl FrameCounter {
    /**
    * interval: microseconds
    **/
    pub fn new(interval: u64) -> Self {
        FrameCounter {
            ticks: 0,
            last_reset: get_us(),
            interval: interval
        }
    }

    pub fn tick(&mut self) {
        let now = get_us();
        if now >= self.last_reset + self.interval {
            println!("{} frames", self.ticks);
            
            self.ticks = 0;
            self.last_reset = now;
        }

        self.ticks += 1;
    }
}
