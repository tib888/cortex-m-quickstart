//! PT8211 16 bit stereo DAC (BCK, DIN, WS) on three output pins
//! TDA1311A equivalent

use embedded_hal::digital::OutputPin;
use num_traits::int::*;
//use typenum::{U0, U1, U2, U3, U4, U5, U6, U7, U8, U9, U10, U11, U12, U13, U14, U15, U16};

pub struct Pt8211<CLKPIN, DATAPIN, WSPIN>
where
    CLKPIN: OutputPin,
    DATAPIN: OutputPin,
    WSPIN: OutputPin,
{
    clk: CLKPIN,
    data: DATAPIN,
    ws: WSPIN,
}

impl<CLKPIN, DATAPIN, WSPIN> Pt8211<CLKPIN, DATAPIN, WSPIN>
where
    CLKPIN: OutputPin,
    DATAPIN: OutputPin,
    WSPIN: OutputPin,
{
    pub fn new(clk: CLKPIN, data: DATAPIN, ws: WSPIN) -> Self {
        Pt8211 { clk, data, ws }
    }

    //TODO lets be generic about the bit count of data

    fn send_word<T>(&mut self, intensity: T)
    where
        T: PrimInt,
    {
        //send MSB first
        let n = 8 * core::mem::size_of::<T>() - 1;
        let mut mask = T::one() << n;

        while mask != T::zero() {
            if (intensity & mask) != T::zero() {
                self.data.set_high();
            } else {
                self.data.set_low();
            }

            //clock rising edge stores the bit
            self.clk.set_high();
            mask = mask >> 1;
            self.clk.set_low();
        }
    }

    pub fn stereo<T>(&mut self, intensity_left: T, intensity_right: T)
    where
        T: PrimInt,
    {
        self.ws.set_high();
        self.send_word(intensity_left); //the previous sample sent out at the rising edge of the first clock of this (?)
        self.ws.set_low();
        self.send_word(intensity_right);
        self.ws.set_high();
    }

    pub fn left_mono<T>(&mut self, intensity: T)
    where
        T: PrimInt,
    {
        self.ws.set_high();
        self.send_word(intensity); //the previous sample sent out at the rising edge of the first clock of this (?)
        self.ws.set_low();
        self.clk.set_high();
        self.clk.set_low();
    }

    pub fn right_mono<T>(&mut self, intensity: T)
    where
        T: PrimInt,
    {
        self.ws.set_low();
        self.send_word(intensity);
        self.ws.set_high();
        self.clk.set_high(); //the previous sample sent out at the rising edge of the first clock of this (?)
        self.clk.set_low();
    }
}
