//! If something connected on the AC mains and its voltage and current can be measured with 
//! DAC for a few periods, this module is able to calculate power usage statistics 
//! Important notes: 
//! - if the measurement of voltage and current has differrent sign, lets set the wrong_sign in the constructor
//! - the minimum power can be below zero in case of non resistive (inductive / capacitive) load
//! - there can be noice in calculations and in computations, so use histeresis epsilons on the output

use core::ops::Add;

#[derive(Debug, Copy, Clone)]
pub struct Statistics {
    //pub avg_power: u32,
    pub min_power: i32,
    pub avg_power: i32,
    pub max_power: i32,
}

pub struct AcSense<DURATION> {
    wrong_sign: bool,
    period: DURATION,
    full_duration: DURATION,
    current_sum: u32,
    voltage_sum: u32,
    avg_current: Option<u32>,
    avg_voltage: Option<u32>,
    in_progress: Option<Statistics>,
    statistics: Option<Statistics>,
}

impl<DURATION> AcSense<DURATION>
where
    DURATION:
        Default + PartialOrd + PartialEq + Add<DURATION, Output = DURATION> + Into<u32> + Copy,
{
    //if the curent is measured with wrong sign relative to the voltage, then set wrong_sign = true
    pub fn new(period: DURATION, wrong_sign: bool) -> AcSense<DURATION> {
        AcSense {
            wrong_sign: wrong_sign,
            period: period,
            full_duration: DURATION::default(), //=sum(dt)
            current_sum: 0,                     //=sum(dt*I)
            voltage_sum: 0,                     //=sum(dt*U)
            avg_current: None,
            avg_voltage: None,
            in_progress: None,
            statistics: None,
        }
    }

    pub fn reset(&mut self) {
        self.full_duration = DURATION::default();
        self.current_sum = 0;
        self.voltage_sum = 0;
        self.avg_current = None;
        self.avg_voltage = None;
        self.in_progress = None;
        self.statistics = None;
    }

    /// this should be called regurarily
    pub fn update(&mut self, current: u32, voltage: u32, delta: DURATION) {
        self.full_duration = self.full_duration + delta;

        let dt: u32 = delta.into();
        self.current_sum += current * dt;
        self.voltage_sum += voltage * dt;

        if let (Some(u0), Some(i0)) = (self.avg_voltage, self.avg_current) {
            let u = (voltage as i32) - (u0 as i32);
            let i = (current as i32) - (i0 as i32);
            let power = u * i;
            let power = if self.wrong_sign { -power } else { power };
            let dp = power * (dt as i32) / (self.period.into() as i32);

            self.in_progress = Some(if let Some(last) = self.in_progress {
                Statistics {
                    min_power: if power < last.min_power {
                        power
                    } else {
                        last.min_power
                    },
                    avg_power: last.avg_power + dp,
                    max_power: if power > last.max_power {
                        power
                    } else {
                        last.max_power
                    },
                }
            } else {
                Statistics {
                    min_power: power,
                    avg_power: dp,
                    max_power: power,
                }
            });
        }

        if self.full_duration > self.period {
            let period = self.full_duration.into();
            self.avg_current = Some(self.current_sum / period);
            self.avg_voltage = Some(self.voltage_sum / period);

            self.statistics = self.in_progress.map(|v| Statistics {
                avg_power: v.avg_power,
                ..v
            });
            self.in_progress = None;

            self.current_sum = 0;
            self.voltage_sum = 0;
            self.full_duration = DURATION::default();
        };
    }

    pub fn state<'a>(&'a self) -> &'a Option<Statistics> {
        &self.statistics
    }
}
