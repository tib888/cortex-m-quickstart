//#![deny(unsafe_code)]
#![deny(warnings)]
#![no_std]

extern crate cortex_m;
extern crate embedded_hal;
extern crate ir;
extern crate stm32f103xx_hal;

pub mod ac_switch;
pub mod dac;
pub mod display;
pub mod floor_heating;
pub mod ir_remote;
pub mod light_control;
pub mod menu;
pub mod pump;
pub mod rgb;
pub mod time;
pub mod valve;
pub mod week_time;

// #[cfg(test)]
// mod test {
//     //use crate::super::*;
//     #[test]
//     fn dummy() {
//         assert_eq!(0, 0);
//     }
// }
