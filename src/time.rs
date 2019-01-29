use core::cmp::Ordering;
use core::marker::PhantomData;
use core::ops::Add;
use core::ops::Sub;
use cortex_m::peripheral::DCB;
use cortex_m::peripheral::DWT;
//use num_traits::Num;
use stm32f103xx_hal::rcc::Clocks;

pub struct Ticker {
    pub frequency: u32, // herz
                        //to_us: u32,         // frequency / 1_000_000
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

    pub fn now(&self) -> Time<SysTicks> {
        Time::<SysTicks> {
            instant: DWT::get_cycle_count(),
            unit: PhantomData::<SysTicks>,
        }
    }
}

/// Time unit marker, contains the tick frequency
#[derive(Copy, Clone, Default)]
pub struct SysTicks;

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
    count: u32,
    unit: PhantomData<UNIT>,
}

#[derive(Copy, Clone)]
pub struct Time<UNIT> {
    instant: u32,
    unit: PhantomData<UNIT>,
}

impl<UNIT> PartialEq for Time<UNIT> {
    fn eq(&self, other: &Time<UNIT>) -> bool {
        self.instant == other.instant
    }
}

impl<UNIT> PartialOrd for Time<UNIT> {
    fn partial_cmp(&self, other: &Time<UNIT>) -> Option<Ordering> {
        self.instant.partial_cmp(&other.instant)
    }
}

impl Time<Seconds> {
    pub fn from_dhms(days: u32, hours: u32, minutes: u32, seconds: u32) -> Time<Seconds> {
        Time::<Seconds> {
            instant: days * 24 * 3600 + hours * 3600 + minutes * 60 + seconds,
            unit: PhantomData::<Seconds>,
        }
    }
}

impl<UNIT> From<u32> for Duration<UNIT> {
    fn from(count: u32) -> Duration<UNIT> {
        Duration::<UNIT> {
            count: count,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl Duration<Seconds> {
    pub fn from_hms(hours: u32, minutes: u32, seconds: u32) -> Duration<Seconds> {
        Duration::<Seconds> {
            count: hours * 3600 + minutes * 60 + seconds,
            unit: PhantomData::<Seconds>,
        }
    }

    pub fn to_hms(&self) -> (u32, u8, u8) {
        let t = self.count;
        let hour = t / 3600;
        let t = t - hour * 3600;
        let min = t / 60;
        let sec = t - min * 60;
        (hour, min as u8, sec as u8)
    }
}

/*
impl Duration<SysTicks> {
    pub fn from_s(s: Duration<Seconds>, tick_frequency: u32) -> Duration<SysTicks> {
        Duration::<SysTicks> {
            count: s.count * tick_frequency,
            unit: PhantomData::<SysTicks>,
        }
    }

    pub fn from_ms(ms: Duration<MilliSeconds>, tick_frequency: u32) -> Duration<SysTicks> {
        Duration::<SysTicks> {
            count: ms.count * tick_frequency / 1_000u32,
            unit: PhantomData::<SysTicks>,
        }
    }

    pub fn from_us(us: Duration<MicroSeconds>, tick_frequency: u32) -> Duration<SysTicks> {
        Duration::<SysTicks> {
            count: us.count * tick_frequency / 1_000_000u32,
            unit: PhantomData::<SysTicks>,
        }
    }
}
*/

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

#[derive(Clone, Copy, Default)]
pub struct WeekTime {
    pub sec: u8,
    pub min: u8,
    pub hour: u8,
    pub weekday: u8,
}

impl From<Time<Seconds>> for WeekTime {
    /// t must be given in seconds from Monday 00:00
    fn from(time: Time<Seconds>) -> Self {
        let t = time.instant;
        let day = t / (60 * 60 * 24);
        let t = t - day * (60 * 60 * 24);
        let hour = t / (60 * 60);
        let t = t - hour * (60 * 60);
        let min = t / 60;
        let sec = t - min * 60;
        let weekday = day % 7;

        WeekTime {
            sec: sec as u8,
            min: min as u8,
            hour: hour as u8,
            weekday: weekday as u8,
        }
    }
}

impl From<WeekTime> for Time<Seconds> {
    /// returns seconds from Monday 00:00
    fn from(original: WeekTime) -> Time<Seconds> {
        Time::<Seconds> {
            instant: original.weekday as u32 * (24 * 60 * 60)
                + original.hour as u32 * (60 * 60)
                + original.min as u32 * 60
                + original.sec as u32,
            unit: PhantomData::<Seconds>,
        }
    }
}

pub trait U32Ext {
    /// Wrap in `Seconds`
    fn s(self) -> Duration<Seconds>;

    /// Wrap in `Milliseconds`
    fn ms(self) -> Duration<MilliSeconds>;

    /// Wrap in `Microseconds`
    fn us(self) -> Duration<MicroSeconds>;
}

impl U32Ext for u32 {
    fn s(self) -> Duration<Seconds> {
        Duration::<Seconds>::from(self)
    }

    fn ms(self) -> Duration<MilliSeconds> {
        Duration::<MilliSeconds>::from(self)
    }

    fn us(self) -> Duration<MicroSeconds> {
        Duration::<MicroSeconds>::from(self)
    }
}
