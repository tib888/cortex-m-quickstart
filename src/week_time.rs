#[derive(Clone, Copy)]
pub struct WeekTime {
    pub sec: u8,
    pub min: u8,
    pub hour: u8,
    pub weekday: u8,
}

impl WeekTime {
    /// t must be given in seconds from Monday 00:00
    pub fn new(t: u32) -> Self {
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
