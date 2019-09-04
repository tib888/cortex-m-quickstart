//! RGB led on 3 output pins

use embedded_hal::digital::v1::OutputPin;

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

impl<RPIN, GPIN, BPIN> RgbLed<RPIN, GPIN, BPIN>
where
    RPIN: OutputPin,
    GPIN: OutputPin,
    BPIN: OutputPin,
{
    pub fn new(r: RPIN, g: GPIN, b: BPIN) -> Self {
        RgbLed { r, g, b }
    }

    pub fn set(&mut self, r: bool, g: bool, b: bool) {
        if !r {
            self.r.set_high();
        } else {
            self.r.set_low();
        };
        if !g {
            self.g.set_high();
        } else {
            self.g.set_low();
        };
        if !b {
            self.b.set_high();
        } else {
            self.b.set_low();
        };
    }

    pub fn color(&mut self, color: Colors) {
        self.raw_color(color as u32);
    }

    pub fn raw_color(&mut self, col: u32) {
        let c = col as u32;
        self.set(
            (c & 0x000080) != 0,
            (c & 0x008000) != 0,
            (c & 0x800000) != 0,
        );
    }
}
