//! Read the NEC IR remote commands on A15 GPIO as input with internal pullup
//#![deny(unsafe_code)]
//#![deny(warnings)]
#![no_main]
#![no_std]

use panic_halt as _;

use cortex_m;
#[macro_use]
use cortex_m_rt;
use cortex_m_semihosting;
use embedded_hal;
use nb;
use panic_halt;
use room_pill;
use stm32f1xx_hal;

use embedded_hal::{
	digital::v2::{InputPin},
};
use cortex_m_rt::{entry, exception, ExceptionFrame};
use cortex_m_semihosting::hio;
use stm32f1xx_hal::{ prelude::*, watchdog::IndependentWatchdog, };
use core::fmt::Write;
use room_pill::{
	ir,
	ir::NecReceiver,
	timing::{Ticker},
};

#[entry]
fn main() -> ! {    
    let device = stm32f1xx_hal::pac::Peripherals::take().unwrap();
    let mut watchdog = IndependentWatchdog::new(device.IWDG);
    watchdog.start(stm32f1xx_hal::time::U32Ext::ms(2_000u32));
    
    let core = cortex_m::Peripherals::take().unwrap();
    let mut rcc = device.RCC.constrain();
    let mut gpioa = device.GPIOA.split(&mut rcc.apb2);
	let gpiob = device.GPIOB.split(&mut rcc.apb2);
    
    let mut afio = device.AFIO.constrain(&mut rcc.apb2);

    // Disables the JTAG to free up pb3, pb4 and pa15 for normal use
    let (pa15, _pb3_itm_swo, _pb4) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);
    
    let ir_receiver = pa15.into_pull_up_input(&mut gpioa.crh);

    let mut flash = device.FLASH.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let tick = Ticker::new(core.DWT, core.DCB, clocks);

    let mut receiver = ir::IrReceiver::new();

    loop {
        let now = tick.now();
        let ir_cmd = receiver.receive(ir_receiver.is_low().unwrap(), now, |last| {
			tick.to_us(now - last).into()
		});
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

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault at {:#?}", ef);
}

#[exception]
fn DefaultHandler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}
