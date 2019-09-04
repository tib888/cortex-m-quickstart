use embedded_hal::digital::v1::{OutputPin, StatefulOutputPin};

pub trait Valve {
    fn open(&mut self);
    fn close(&mut self);
}

pub trait StatefulValve: Valve {
    fn opened(&self) -> bool;
    fn closed(&self) -> bool {
        !self.opened()
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
    fn open(&mut self) {
        self.pin.set_low();
    }
    fn close(&mut self) {
        self.pin.set_high();
    }
}

impl<PIN> StatefulValve for ValveSSR<PIN>
where
    PIN: OutputPin + StatefulOutputPin,
{
    fn opened(&self) -> bool {
        self.pin.is_set_low()
    }
}
