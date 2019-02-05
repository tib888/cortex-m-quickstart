use core::cmp::Ordering;
use core::marker::PhantomData;
use core::ops::Add;
use core::ops::Sub;
use cortex_m::peripheral::DCB;
use cortex_m::peripheral::DWT;
//use num_traits::Num;
use num_traits::*;
use stm32f103xx_hal::rcc::Clocks;

/// Time unit marker, implies the tick frequency
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
pub struct Duration<T, UNIT>
where
    T: Ord,
{
    count: T,
    unit: PhantomData<UNIT>,
}

#[derive(Copy, Clone)]
pub struct Time<T, UNIT>
where
    T: Ord,
{
    instant: T,
    unit: PhantomData<UNIT>,
}

pub trait TimeSource<T, UNIT>
where
    T: Ord,
{
    fn now(&self) -> Time<T, UNIT>;
    fn from_s(&self, duration: Duration<T, Seconds>) -> Duration<T, UNIT>;
    fn from_ms(&self, duration: Duration<T, MilliSeconds>) -> Duration<T, UNIT>;
    fn from_us(&self, duration: Duration<T, MicroSeconds>) -> Duration<T, UNIT>;
    //fn ellapsed_between(now: Time<T, UNIT>, past: Time<T, UNIT>) -> Duration<T, UNIT>;
}

impl<T, UNIT> PartialEq for Time<T, UNIT>
where
    T: Ord,
{
    fn eq(&self, other: &Time<T, UNIT>) -> bool {
        self.instant == other.instant
    }
}

impl<T, UNIT> PartialOrd for Time<T, UNIT>
where
    T: Ord,
{
    fn partial_cmp(&self, other: &Time<T, UNIT>) -> Option<Ordering> {
        self.instant.partial_cmp(&other.instant)
    }
}

impl Time<u32, Seconds> where {
    pub fn from_dhms(days: u32, hours: u32, minutes: u32, seconds: u32) -> Time<u32, Seconds> {
        Time::<u32, Seconds> {
            instant: days * (24 * 3600) + hours * 3600 + minutes * 60 + seconds,
            unit: PhantomData::<Seconds>,
        }
    }
}

impl<UNIT> From<u32> for Duration<u32, UNIT> {
    fn from(count: u32) -> Duration<u32, UNIT> {
        Duration::<u32, UNIT> {
            count: count,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl Duration<u32, Seconds> {
    pub fn from_hms(hours: u32, minutes: u32, seconds: u32) -> Duration<u32, Seconds> {
        Duration::<u32, Seconds> {
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

impl<T, UNIT> PartialOrd for Duration<T, UNIT>
where
    T: Ord,
{
    fn partial_cmp(&self, other: &Duration<T, UNIT>) -> Option<Ordering> {
        Some(self.count.cmp(&other.count))
    }
}

impl<T, UNIT> Ord for Duration<T, UNIT>
where
    T: Ord,
{
    fn cmp(&self, other: &Duration<T, UNIT>) -> Ordering {
        self.count.cmp(&other.count)
    }
}

impl<T, UNIT> PartialEq for Duration<T, UNIT>
where
    T: PartialEq + Ord,
{
    fn eq(&self, other: &Duration<T, UNIT>) -> bool {
        self.count == other.count
    }
}

impl<T, UNIT> Eq for Duration<T, UNIT> where T: Ord {}

impl<T, UNIT> Default for Duration<T, UNIT>
where
    T: Default + Ord,
{
    fn default() -> Duration<T, UNIT> {
        Duration::<T, UNIT> {
            count: T::default(),
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<T, UNIT> Add for Duration<T, UNIT>
where
    T: Add<Output = T> + Ord,
{
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::Output {
            count: self.count + rhs.count,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<T, UNIT> Sub for Duration<T, UNIT>
where
    T: Sub<Output = T> + Ord,
{
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::Output {
            count: self.count - rhs.count,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<T, UNIT> Sub for Time<T, UNIT>
where
    T: WrappingSub + Ord,
{
    type Output = Duration<T, UNIT>;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::Output {
            count: self.instant.wrapping_sub(&rhs.instant),
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<T, UNIT> Add<Duration<T, UNIT>> for Time<T, UNIT>
where
    T: WrappingAdd + Ord,
{
    type Output = Self;
    fn add(self, rhs: Duration<T, UNIT>) -> Self::Output {
        Self::Output {
            instant: self.instant.wrapping_add(&rhs.count),
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<T, UNIT> Sub<Duration<T, UNIT>> for Time<T, UNIT>
where
    T: WrappingSub + Ord,
{
    type Output = Self;
    fn sub(self, rhs: Duration<T, UNIT>) -> Self::Output {
        Self::Output {
            instant: self.instant.wrapping_sub(&rhs.count),
            unit: PhantomData::<UNIT>,
        }
    }
}

pub trait U32Ext {
    /// Wrap in `Seconds`
    fn s(self) -> Duration<u32, Seconds>;

    /// Wrap in `Milliseconds`
    fn ms(self) -> Duration<u32, MilliSeconds>;

    /// Wrap in `Microseconds`
    fn us(self) -> Duration<u32, MicroSeconds>;
}

impl U32Ext for u32 {
    fn s(self) -> Duration<u32, Seconds> {
        Duration::<u32, Seconds>::from(self)
    }

    fn ms(self) -> Duration<u32, MilliSeconds> {
        Duration::<u32, MilliSeconds>::from(self)
    }

    fn us(self) -> Duration<u32, MicroSeconds> {
        Duration::<u32, MicroSeconds>::from(self)
    }
}

#[derive(Clone, Copy, Default)]
pub struct WeekTime {
    pub sec: u8,
    pub min: u8,
    pub hour: u8,
    pub weekday: u8,
}

impl From<Time<u32, Seconds>> for WeekTime {
    /// t must be given in seconds from Monday 00:00
    fn from(time: Time<u32, Seconds>) -> Self {
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

impl From<WeekTime> for Time<u32, Seconds> {
    /// returns seconds from Monday 00:00
    fn from(original: WeekTime) -> Time<u32, Seconds> {
        Time::<u32, Seconds> {
            instant: original.weekday as u32 * (24 * 60 * 60)
                + original.hour as u32 * (60 * 60)
                + original.min as u32 * 60
                + original.sec as u32,
            unit: PhantomData::<Seconds>,
        }
    }
}

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
}

impl TimeSource<u32, SysTicks> for Ticker {
    fn now(&self) -> Time<u32, SysTicks> {
        Time::<u32, SysTicks> {
            instant: DWT::get_cycle_count(),
            unit: PhantomData::<SysTicks>,
        }
    }

    fn from_s(&self, duration: Duration<u32, Seconds>) -> Duration<u32, SysTicks> {
        Duration::<u32, SysTicks> {
            count: duration.count * self.frequency,
            unit: PhantomData::<SysTicks>,
        }
    }

    fn from_ms(&self, duration: Duration<u32, MilliSeconds>) -> Duration<u32, SysTicks> {
        Duration::<u32, SysTicks> {
            count: duration.count * (self.frequency / 1_000),
            unit: PhantomData::<SysTicks>,
        }
    }

    fn from_us(&self, duration: Duration<u32, MicroSeconds>) -> Duration<u32, SysTicks> {
        Duration::<u32, SysTicks> {
            count: duration.count * (self.frequency / 1_000_000),
            unit: PhantomData::<SysTicks>,
        }
    }
}
