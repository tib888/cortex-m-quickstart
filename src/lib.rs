#![deny(unsafe_code)]
#![deny(warnings)]
#![no_std]

extern crate cortex_m;
extern crate embedded_hal;
extern crate ir;
extern crate stm32f103xx_hal;

pub mod floor_heating;
pub mod light_control;
pub mod pump;
pub mod rgb;
pub mod ticker;
pub mod valve;
