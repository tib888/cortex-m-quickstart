#![deny(unsafe_code)]
#![deny(warnings)]
#![no_std]

extern crate cortex_m;
extern crate embedded_hal;

pub mod floor_heating;
pub mod light_control;
pub mod pump;
pub mod rgb;
pub mod ticker;
pub mod valve;
