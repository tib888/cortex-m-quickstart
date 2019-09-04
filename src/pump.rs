use panic_halt as _;
use embedded_hal::digital::v2::{OutputPin, StatefulOutputPin};

pub trait Pump {
    type Error;
    fn start(&mut self) -> Result<(), Self::Error>;
    fn stop(&mut self) -> Result<(), Self::Error>;
}

pub trait StatefulPump: Pump {    
    fn started(&self) -> Result<bool, Self::Error>;
    fn stopped(&self) -> Result<bool, Self::Error> {
        self.started().map(|started| !started)
    }
}

pub struct PumpSSR<PIN>
where
    PIN: OutputPin,
{
    pin: PIN,
}

impl<PIN> PumpSSR<PIN>
where
    PIN: OutputPin,
{
    pub fn new(pin: PIN) -> Self {
        PumpSSR { pin }
    }
}

impl<PIN> Pump for PumpSSR<PIN>
where
    PIN: OutputPin,
{
    type Error = PIN::Error;

    fn start(&mut self) -> Result<(), Self::Error> {
        self.pin.set_low()
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.pin.set_high()
    }
}

impl<PIN> StatefulPump for PumpSSR<PIN>
where
    PIN: OutputPin + StatefulOutputPin,
{
    fn started(&self) -> Result<bool, Self::Error> {
        self.pin.is_set_low()
    }
}
