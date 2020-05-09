//! Read the temperature from DS18B20 1-wire temperature sensors connected to B4 GPIO
//! JTAG is removed from B3, B4 to make it work
//#![deny(unsafe_code)]
//#![deny(warnings)]
#![no_std]
#![no_main]

use cortex_m;
use cortex_m_semihosting;
#[macro_use]
use cortex_m_rt;
use onewire;
use panic_halt as _;
use stm32f1xx_hal;

use stm32f1xx_hal::{ delay::Delay, prelude::* };
use cortex_m_rt::{entry, exception, ExceptionFrame};
use cortex_m_semihosting::hio;
use core::fmt::Write;

use onewire::{*, ds18x20::* };

#[entry]
fn main() -> ! {
    let mut hstdout = hio::hstdout().unwrap();

    let core = cortex_m::Peripherals::take().unwrap();
    let device = stm32f1xx_hal::pac::Peripherals::take().unwrap();

    let mut flash = device.FLASH.constrain();

    //flash.acr.prftbe().enabled();//?? Configure Flash prefetch - Prefetch buffer is not available on value line devices
    //scb().set_priority_grouping(NVIC_PRIORITYGROUP_4);

    let mut rcc = device.RCC.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(72.mhz())
        .hclk(72.mhz())
        .pclk1(36.mhz())
        .pclk2(72.mhz())
        .freeze(&mut flash.acr);

    let gpioa = device.GPIOA.split(&mut rcc.apb2);
	let mut gpiob = device.GPIOB.split(&mut rcc.apb2);
	let mut gpioc = device.GPIOC.split(&mut rcc.apb2);

    let mut afio = device.AFIO.constrain(&mut rcc.apb2);
    // Disables the JTAG to free up pb3, pb4 and pa15 for normal use
    let (_pa15, _pb3_itm_swo, pb4) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);
    
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    let mut one_wire = {
		// DS18B20 1-wire temperature sensors connected to B4 GPIO
		let onewire_io = pb4.into_open_drain_output(&mut gpiob.crl);
		let delay = Delay::new(core.SYST, clocks);
		OneWirePort::new(onewire_io, delay).unwrap()
	};

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
                led.toggle().unwrap();
            }
        }

        it.reset(0);
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