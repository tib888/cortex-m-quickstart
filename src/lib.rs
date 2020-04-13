//#![deny(unsafe_code)]
//#![deny(warnings)]
#![no_std]

pub mod ac_sense;
pub mod ac_switch;
pub mod dac;
pub mod display;
pub mod floor_heating;
pub mod ir;
pub mod ir_remote;
pub mod light_control;
pub mod menu;
pub mod pump;
pub mod rgb;
pub mod roll;
pub mod timing;
pub mod valve;

// #[cfg(test)]
// mod test {
//     //use crate::super::*;
//     #[test]
//     fn dummy() {
//         assert_eq!(0, 0);
//     }
// }
