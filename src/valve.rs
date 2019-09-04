use embedded_hal::digital::v2::{OutputPin, StatefulOutputPin};

pub trait Valve {
    type Error;
    fn open(&mut self) -> Result<(), Self::Error>;
    fn close(&mut self) -> Result<(), Self::Error>;
}

pub trait StatefulValve: Valve {
    fn opened(&self) -> Result<bool, Self::Error>;
    fn closed(&self) -> Result<bool, Self::Error> {
        self.opened().map(|opened| !opened)
    }
}

pub struct ValveSSR<PIN>
where
    PIN: OutputPin,
{
    pin: PIN,
}

impl<PIN> ValveSSR<PIN>
where
    PIN: OutputPin,
{
    pub fn new(pin: PIN) -> Self {
        ValveSSR { pin }
    }
}

impl<PIN> Valve for ValveSSR<PIN>
where
    PIN: OutputPin,
{
    type Error = PIN::Error;
    fn open(&mut self) -> Result<(), Self::Error> {
        self.pin.set_low()
    }
    fn close(&mut self) -> Result<(), Self::Error> {
        self.pin.set_high()
    }
}

impl<PIN> StatefulValve for ValveSSR<PIN>
where
    PIN: OutputPin + StatefulOutputPin,
{
    fn opened(&self) -> Result<bool, Self::Error> {
        self.pin.is_set_low()
    }
}
