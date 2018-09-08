use embedded_hal::digital::OutputPin;
use embedded_hal::digital::StatefulOutputPin;

pub trait Pump {
    fn start(&mut self);
    fn stop(&mut self);
}

pub trait StatefulPump: Pump {
    fn started(&self) -> bool;
    fn stopped(&self) -> bool {
        !self.started()
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
    fn start(&mut self) {
        self.pin.set_low();
    }

    fn stop(&mut self) {
        self.pin.set_high();
    }
}

impl<PIN> StatefulPump for PumpSSR<PIN>
where
    PIN: OutputPin + StatefulOutputPin,
{
    fn started(&self) -> bool {
        self.pin.is_set_low()
    }
}
