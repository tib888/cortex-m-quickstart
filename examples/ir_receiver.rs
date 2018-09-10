//! Read the NEC IR remote commands on A15 GPIO as input with internal pullup
//#![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

extern crate cortex_m;
#[macro_use]
extern crate cortex_m_rt as rt;
extern crate cortex_m_semihosting as sh;
extern crate embedded_hal;
extern crate ir;
extern crate nb;
extern crate panic_semihosting;
extern crate room_pill;
extern crate stm32f103xx_hal as hal;

use core::fmt::Write;
use hal::prelude::*;
use hal::stm32f103xx;
use ir::NecReceiver;
use room_pill::time::{Ticker, Ticks, Time};
use rt::ExceptionFrame;
use sh::hio;

entry!(main);

fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32f103xx::Peripherals::take().unwrap();

    let mut rcc = dp.RCC.constrain();
    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let ir_receiver = gpioa.pa15.into_pull_up_input(&mut gpioa.crh);

    let mut flash = dp.FLASH.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let tick = Ticker::new(cp.DWT, cp.DCB, clocks);

    let mut receiver = ir::IrReceiver::<Time<Ticks>>::new();

    //let mut hstdout = hio::hstdout().unwrap();
    //writeln!(hstdout, "started...").unwrap();

    loop {
        let t = tick.now();
        let ir_cmd = receiver.receive(t, ir_receiver.is_low());
        print_ir_command(&ir_cmd);
    }
}

fn print_ir_command(ir_cmd: &nb::Result<ir::NecContent, u32>) {
    match *ir_cmd {
        Ok(ir::NecContent::Repeat) => {
            let mut hstdout = hio::hstdout().unwrap();
            hstdout.write_str("R").unwrap();
        }
        Ok(ir::NecContent::Data(data)) => {
            let mut hstdout = hio::hstdout().unwrap();
            hstdout.write_fmt(format_args!(">{:X} ", data)).unwrap();
        }
        Err(nb::Error::Other(wrong_data)) => {
            let mut hstdout = hio::hstdout().unwrap();
            hstdout
                .write_fmt(format_args!("!{:b} ", wrong_data))
                .unwrap();
        }
        Err(nb::Error::WouldBlock) => {}
    }
}

exception!(HardFault, hard_fault);

fn hard_fault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault at {:#?}", ef);
}

exception!(*, default_handler);

fn default_handler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}
