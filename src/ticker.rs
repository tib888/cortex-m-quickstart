use cortex_m::peripheral::DCB;
use cortex_m::peripheral::DWT;

use ir::Instant;
use stm32f103xx_hal::rcc::Clocks;

#[derive(Copy, Clone)]
pub struct Time {
    now: u32,
}

impl Time {
    /// Ticks elapsed since the `Time` was created
    pub fn elapsed(&self) -> u32 {
        DWT::get_cycle_count().wrapping_sub(self.now)
    }

    pub fn elapsed_till(&self, till: &Self) -> u32 {
        till.now.wrapping_sub(self.now)
    }

    pub fn elapsed_since(&self, since: &Self) -> u32 {
        self.now.wrapping_sub(since.now)
    }

    pub fn shift(&mut self, ticks: u32) {
        self.now.wrapping_add(ticks);
    }
}

pub struct Ticker {
    pub frequency: u32, //herz
}

impl Ticker {
    pub fn new(mut dwt: DWT, mut dcb: DCB, clocks: Clocks) -> Self {
        dcb.enable_trace();
        dwt.enable_cycle_counter();

        // now the CYCCNT counter can't be stopped or resetted
        drop(dwt);

        Ticker {
            frequency: clocks.sysclk().0,
        }
    }

    pub fn now(&self) -> Time {
        Time {
            now: DWT::get_cycle_count(),
        }
    }
}

impl Instant for Time {
    /// called on an older instant, returns the elapsed microseconds until the given now
    fn elapsed_us_till(&self, now: &Self) -> u32 {
        self.elapsed_till(&now) >> 3 //8Mhz clock, so div by 8
    }
}
