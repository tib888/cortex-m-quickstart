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
extern crate stm32f103xx_hal as hal;

use core::fmt::Write;
use cortex_m::peripheral::syst::SystClkSource;
use hal::prelude::*;
use hal::stm32f103xx;
use ir::NecReceiver;
use rt::ExceptionFrame;
use sh::hio;

static mut TICK: u32 = 0;

entry!(main);

fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32f103xx::Peripherals::take().unwrap();

    let mut rcc = dp.RCC.constrain();
    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let ir_receiver = gpioa.pa15.into_pull_up_input(&mut gpioa.crh);

    // configures the system timer to trigger a SysTick exception every half millisecond
    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(4_000); // period = 0.5ms => 2000Hz => 8_000_000 / 2_000 = 4_000	//4500 would be ideal for NEC decoding
    syst.enable_counter();
    syst.enable_interrupt();

    let mut receiver = ir::IrReceiver::new(4_000 / 8); // period = 0.5ms = 500us

    //let mut hstdout = hio::hstdout().unwrap();
    //writeln!(hstdout, "started...").unwrap();

    loop {
        let t = unsafe { TICK };
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

exception!(SysTick, sys_tick);

fn sys_tick() {
    unsafe {
        TICK = TICK + 1;
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
