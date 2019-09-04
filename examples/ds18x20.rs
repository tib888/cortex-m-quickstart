//! Read the temperature from DS18B20 1-wire temperature sensors connected to B4 GPIO
//! JTAG is removed from B3, B4 to make it work
//#![deny(unsafe_code)]
#![deny(warnings)]
#![no_std]
#![no_main]

extern crate cortex_m;
extern crate cortex_m_semihosting as sh;
extern crate embedded_hal;
#[macro_use]
extern crate cortex_m_rt as rt;
extern crate nb;
extern crate onewire;
extern crate panic_halt;
extern crate stm32f1xx_hal as hal;

use crate::hal::delay::Delay;
use crate::hal::prelude::*;
use crate::hal::stm32f1xx;
use crate::rt::entry;
use crate::rt::ExceptionFrame;
use crate::sh::hio;
use core::fmt::Write;

use onewire::ds18x20::*;
use onewire::*;

#[entry]
fn main() -> ! {
    let mut hstdout = hio::hstdout().unwrap();

    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32f1xx::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();

    //flash.acr.prftbe().enabled();//?? Configure Flash prefetch - Prefetch buffer is not available on value line devices
    //scb().set_priority_grouping(NVIC_PRIORITYGROUP_4);

    let mut rcc = dp.RCC.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(72.mhz())
        .hclk(72.mhz())
        .pclk1(36.mhz())
        .pclk2(72.mhz())
        .freeze(&mut flash.acr);

    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);
    // Disables the JTAG to free up pb3, pb4 and pa15 for normal use
    afio.mapr.disable_jtag();

    let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);

    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);

    let delay = Delay::new(cp.SYST, clocks);
    let io = gpiob.pb4.into_open_drain_output(&mut gpiob.crl);
    let mut one_wire = OneWirePort::new(io, delay);

    let mut it = RomIterator::new(0);
    loop {
        match one_wire.iterate_next(true, &mut it) {
            Ok(Some(rom)) => {
                if let Some(_device_type) = detect_18x20_devices(rom[0]) {
                    //writeln!(hstdout, "rom: {:?}", &rom).unwrap();

                    if let Ok(_required_delay) = one_wire.start_temperature_measurement(&rom) {
                        //led.set_high();
                        //TODO nonblocking
                        //one_wire.delay.delay_ms(required_delay);
                        //led.set_low();

                        let temperature = one_wire.read_temperature_measurement_result(&rom);
                        match temperature {
                            Ok(t) => writeln!(
                                hstdout,
                                "T = {} + {}/16 C",
                                t.whole_degrees(),
                                t.fraction_degrees()
                            )
                            .unwrap(),
                            Err(code) => writeln!(hstdout, "Error: {:?}", code).unwrap(),
                        }
                    }
                } else {
                    writeln!(hstdout, "Unknown one wire device.").unwrap();
                }
                continue;
            }

            Err(e) => {
                writeln!(hstdout, "{:?}", &e).unwrap();
            }

            _ => {
                led.toggle();
            }
        }

        it.reset(0);
    }
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}

#[exception]
fn DefaultHandler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}
