//! RGB led on 3 output pins

use embedded_hal::digital::v2::OutputPin;

#[derive(Debug, PartialEq)]
pub enum Colors {
    Black = 0x000000,
    Red = 0x0000FF,
    Green = 0x00FF00,
    Yellow = 0x00FFFF,
    Blue = 0xFF0000,
    Purple = 0xFF00FF,
    Cyan = 0xFFFF00,
    White = 0xFFFFFF,
}

pub trait Rgb {
    type Error;
    fn set(&mut self, r: bool, g: bool, b: bool) -> Result<(), Self::Error>;
    fn raw_color(&mut self, col: u32) -> Result<(), Self::Error>;
    fn color(&mut self, color: Colors) -> Result<(), Self::Error> {
        self.raw_color(color as u32)
    }
}

pub struct RgbLed<RPIN, GPIN, BPIN>
where
    RPIN: OutputPin,
    GPIN: OutputPin,
    BPIN: OutputPin,
{
    r: RPIN,
    g: GPIN,
    b: BPIN,
}

impl<RPIN, GPIN, BPIN, ERROR> RgbLed<RPIN, GPIN, BPIN>
where
    RPIN: OutputPin<Error = ERROR>,
    GPIN: OutputPin<Error = ERROR>,
    BPIN: OutputPin<Error = ERROR>,
{
    pub fn new(r: RPIN, g: GPIN, b: BPIN) -> Self {
        RgbLed { r, g, b }
    }
}

impl<RPIN, GPIN, BPIN, ERROR> Rgb for RgbLed<RPIN, GPIN, BPIN>
where
    RPIN: OutputPin<Error = ERROR>,
    GPIN: OutputPin<Error = ERROR>,
    BPIN: OutputPin<Error = ERROR>,
{
    type Error = ERROR;

    fn set(&mut self, r: bool, g: bool, b: bool) -> Result<(), Self::Error> {
        if !r {
            self.r.set_high()?;
        } else {
            self.r.set_low()?;
        };
        if !g {
            self.g.set_high()?;
        } else {
            self.g.set_low()?;
        };
        if !b {
            self.b.set_high()?;
        } else {
            self.b.set_low()?;
        };
        Ok(())
    }

    fn raw_color(&mut self, col: u32) -> Result<(), Self::Error> {
        let c = col as u32;
        self.set(
            (c & 0x000080) != 0,
            (c & 0x008000) != 0,
            (c & 0x800000) != 0,
        )
    }
}
