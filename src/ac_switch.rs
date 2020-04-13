//! Switched AC mains optically decopuled and pull down the input pin with internal pullup
//! if there is no low pulse on the input for more than one AC period, the state should
//! change to off

use core::ops::Add;
use embedded_hal::digital::v2::InputPin;

#[derive(Copy, Clone, PartialEq)]
pub enum OnOff {
    Off,
    On,
}

pub struct AcSwitch<PIN, DURATION>
where
    PIN: InputPin,
    DURATION: Default + PartialOrd + Add,
{
    pin: PIN,
    period: DURATION,
    full_duration: DURATION,
    low_duration: DURATION,
    last: Option<OnOff>,
    current: Option<OnOff>,
}

impl<PIN, DURATION> AcSwitch<PIN, DURATION>
where
    PIN: InputPin,
    DURATION: Default + PartialOrd + PartialEq + Add<DURATION, Output = DURATION> + Copy,
{
    pub fn new(pin: PIN, period: DURATION) -> AcSwitch<PIN, DURATION> {
        AcSwitch {
            pin: pin,
            period: period,
            full_duration: DURATION::default(),
            low_duration: DURATION::default(),
            last: Option::None,
            current: Option::None,
        }
    }

    /// this should be called regurarily
    pub fn update(&mut self, delta: DURATION) -> Result<(), PIN::Error> {
        self.full_duration = self.full_duration + delta;

        if self.pin.is_low()? {
            self.low_duration = self.low_duration + delta;
        }

        if self.full_duration >= self.period {
            self.last = self.current;
            self.current = Some(if self.low_duration == DURATION::default() {
                OnOff::Off
            } else {
                OnOff::On
            });

            self.full_duration = DURATION::default();
            self.low_duration = DURATION::default();
        };

        Ok(())
    }

    pub fn state(&self) -> Option<OnOff> {
        self.current
    }

    pub fn last_state(&self) -> Option<OnOff> {
        self.last
    }
}
