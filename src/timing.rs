use core::cmp::Ordering;
use core::ops::{Add, Sub};
use core::convert::From;
use num_traits::{Num, WrappingSub, WrappingAdd};
use core::marker::PhantomData;
use cortex_m::peripheral::{DCB, DWT};
use stm32f1xx_hal::rcc::Clocks;

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

/// Time unit marker
#[derive(Copy, Clone, Default)]
pub struct NanoSeconds;

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

impl<T> Time<T, Seconds> 
where 
    T: Num + Ord + From<u32>,
{
    pub fn from_dhms(days: T, hours: T, minutes: T, seconds: T) -> Time<T, Seconds> {
        Time::<T, Seconds> {
            instant: days * T::from(24u32 * 3600u32) + hours * T::from(3600u32) + minutes * T::from(60u32) + seconds,
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

impl<UNIT> From<u64> for Duration<u64, UNIT> {
    fn from(count: u64) -> Duration<u64, UNIT> {
        Duration::<u64, UNIT> {
            count: count,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<UNIT> From<Duration<u64, UNIT>> for Duration<u32, UNIT> {
    fn from(duration: Duration<u64, UNIT>) -> Duration<u32, UNIT> {
        Duration::<u32, UNIT> {
            count: duration.count as u32,
            unit: PhantomData::<UNIT>,
        }
    }
}

impl<T> Duration<T, Seconds> 
where 
    T: Num + Ord + From<u32> + Copy,
{
    pub fn from_hms(hours: T, minutes: T, seconds: T) -> Duration<T, Seconds> {
        Duration::<T, Seconds> {
            count: hours * T::from(3600u32) + minutes * T::from(60u32) + seconds,
            unit: PhantomData::<Seconds>,
        }
    }

    pub fn to_hms(&self) -> (T, T, T) {
        let t = self.count;
        let hour = t / T::from(3600u32);
        let t = t - hour * T::from(3600u32);
        let min = t / T::from(60u32);
        let sec = t - min * T::from(60u32);
        (hour, min, sec)
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

pub trait TimeExt<T> 
where 
    T: Ord 
{
    /// Wrap in `Seconds`
    fn s(self) -> Duration<T, Seconds>;

    /// Wrap in `Milliseconds`
    fn ms(self) -> Duration<T, MilliSeconds>;

    /// Wrap in `Microseconds`
    fn us(self) -> Duration<T, MicroSeconds>;

    /// Wrap in `Nanoseconds`
    fn ns(self) -> Duration<T, NanoSeconds>;
}

impl TimeExt<u32> for u32 {
    fn s(self) -> Duration<u32, Seconds> {
        Duration::<u32, Seconds>::from(self)
    }

    fn ms(self) -> Duration<u32, MilliSeconds> {
        Duration::<u32, MilliSeconds>::from(self)
    }

    fn us(self) -> Duration<u32, MicroSeconds> {
        Duration::<u32, MicroSeconds>::from(self)
    }

    fn ns(self) -> Duration<u32, NanoSeconds> {
        Duration::<u32, NanoSeconds>::from(self)
    }
}

impl TimeExt<u64> for u64 {
    fn s(self) -> Duration<u64, Seconds> {
        Duration::<u64, Seconds>::from(self)
    }

    fn ms(self) -> Duration<u64, MilliSeconds> {
        Duration::<u64, MilliSeconds>::from(self)
    }

    fn us(self) -> Duration<u64, MicroSeconds> {
        Duration::<u64, MicroSeconds>::from(self)
    }

    fn ns(self) -> Duration<u64, NanoSeconds> {
        Duration::<u64, NanoSeconds>::from(self)
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
    pub frequency: u32, // in Hz
    pub period_x: u64, // in 2^19 period in us (7282 in case of 72Mhz)
}

impl Ticker {
    pub fn new(mut dwt: DWT, mut dcb: DCB, clocks: Clocks) -> Self {
        dcb.enable_trace();
        dwt.enable_cycle_counter();

        // now the CYCCNT counter can't be stopped or resetted
        drop(dwt);

        let clk = clocks.sysclk().0 as u64;

        Ticker {
            frequency: clocks.sysclk().0,
            period_x: (((1u64 << 19) * 1_000_000u64) + (clk >> 1)) / clk,
        }
    }
}

impl TimeSource<u64, MicroSeconds> for Ticker {
    fn now(&self) -> Time<u64, MicroSeconds> {
        Time::<u64, MicroSeconds> {
            instant: (DWT::get_cycle_count() as u64 * self.period_x) >> 19, 
            unit: PhantomData::<MicroSeconds>,
        }
    }
}
