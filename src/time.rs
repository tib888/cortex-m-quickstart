use core::cmp::Ordering;
use core::marker::PhantomData;
use core::ops::Add;
use core::ops::Sub;
use cortex_m::peripheral::DCB;
use cortex_m::peripheral::DWT;

use ir::Instant;
use stm32f103xx_hal::rcc::Clocks;

// impl Time {
//     /// Ticks elapsed since the `Time` was created
//     pub fn elapsed(&self) -> Duration<Ticks> {
//         Duration<Ticks> {
//             count: DWT::get_cycle_count().wrapping_sub(self.now)
//             unit: Ticks
//         }
//     }
// }

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

    pub fn now(&self) -> Time<Ticks> {
        Time::<Ticks> {
            instant: DWT::get_cycle_count(),
            unit: PhantomData::<Ticks>,
        }
    }
}

impl Instant for Time<Ticks> {
    /// called on an older instant, returns the elapsed microseconds until the given now
    fn elapsed_us_till(&self, now: &Self) -> u32 {
        now.instant.wrapping_sub(self.instant) >> 3 //8Mhz clock, so div by 8
    }

    // fn elapsed_us_till(&self, now: &Self) -> Duration<MicroSeconds> {
    //     self.elapsed_till(&now) >> 3 //8Mhz clock, so div by 8
    // }
}

/// Time unit marker
#[derive(Copy, Clone, Default)]
pub struct Ticks;

/// Time unit marker
#[derive(Copy, Clone, Default)]
pub struct Seconds;

/// Time unit marker
#[derive(Copy, Clone, Default)]
pub struct MilliSeconds;

/// Time unit marker
#[derive(Copy, Clone, Default)]
pub struct MicroSeconds;

#[derive(Copy, Clone)]
pub struct Duration<UNIT> {
    pub count: u32,
    pub unit: PhantomData<UNIT>,
}

#[derive(Copy, Clone)]
pub struct Time<UNIT> {
    pub instant: u32,
    pub unit: PhantomData<UNIT>,
}

impl<UNIT> PartialEq for Time<UNIT> {
    fn eq(&self, other: &Time<UNIT>) -> bool {
        self.instant == other.instant
    }
}

impl<UNIT> From<u32> for Time<UNIT> {
    fn from(original: u32) -> Time<UNIT> {
        Time::<UNIT> {
            instant: original,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl From<u32> for Duration<Ticks> {
    fn from(original: u32) -> Duration<Ticks> {
        Duration::<Ticks> {
            count: original,
            unit: PhantomData::<Ticks>,
        }
    }
}

impl Duration<Seconds> {
    pub fn hms(hours: u32, minutes: u32, seconds: u32) -> Duration<Seconds> {
        Duration::<Seconds> {
            count: hours * 3600 + minutes * 60 + seconds,
            unit: PhantomData::<Seconds>,
        }
    }

    pub fn sec(seconds: u32) -> Duration<Seconds> {
        Duration::<Seconds> {
            count: seconds,
            unit: PhantomData::<Seconds>,
        }
    }
}

impl<UNIT> PartialOrd for Duration<UNIT> {
    fn partial_cmp(&self, other: &Duration<UNIT>) -> Option<Ordering> {
        Some(self.count.cmp(&other.count))
    }
}

impl<UNIT> Ord for Duration<UNIT> {
    fn cmp(&self, other: &Duration<UNIT>) -> Ordering {
        self.count.cmp(&other.count)
    }
}

impl<UNIT> PartialEq for Duration<UNIT> {
    fn eq(&self, other: &Duration<UNIT>) -> bool {
        self.count == other.count
    }
}

impl<UNIT> Eq for Duration<UNIT> {}

impl<UNIT> Default for Duration<UNIT> {
    fn default() -> Duration<UNIT> {
        Duration::<UNIT> {
            count: 0,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<UNIT> Add for Duration<UNIT> {
    type Output = Duration<UNIT>;
    fn add(self, rhs: Self) -> Self::Output {
        Duration::<UNIT> {
            count: self.count + rhs.count,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<UNIT> Sub for Duration<UNIT> {
    type Output = Duration<UNIT>;
    fn sub(self, rhs: Self) -> Self::Output {
        Duration::<UNIT> {
            count: self.count - rhs.count,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<UNIT> Sub for Time<UNIT> {
    type Output = Duration<UNIT>;
    fn sub(self, rhs: Self) -> Self::Output {
        Duration::<UNIT> {
            count: self.instant.wrapping_sub(rhs.instant),
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<UNIT> Add<Duration<UNIT>> for Time<UNIT> {
    type Output = Time<UNIT>;
    fn add(self, rhs: Duration<UNIT>) -> Self::Output {
        Time::<UNIT> {
            instant: self.instant.wrapping_add(rhs.count),
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<UNIT> Sub<Duration<UNIT>> for Time<UNIT> {
    type Output = Time<UNIT>;
    fn sub(self, rhs: Duration<UNIT>) -> Self::Output {
        Time::<UNIT> {
            instant: self.instant.wrapping_sub(rhs.count),
            unit: PhantomData::<UNIT>,
        }
    }
}
