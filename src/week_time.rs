use core::marker::PhantomData;
use crate::time::{Seconds, Time};

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
