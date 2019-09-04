//! DS18B20 1-wire temperature sensors connected to B4 GPIO
//! JTAG is removed from B3, B4 to make it work
//!
//! Read the NEC IR remote commands on A15 GPIO as input with internal pullup
//!
//! RGB led on PB13, PB14, PB15 as push pull output
//!
//! Solid state relay 1 connected to B6 drives the valve
//! Solid state relay 2 connected to B7 drives the pump
//!
//! Hx1230 lcd display conected to SPI1 and some gpio port:
//!   PA5 = Display SPI clock
//!   PA6 = Display SPI input - not used
//!   PA7 = Display SPI data
//!   PA2 = Display Chip Select^ - if SPI is not shared this could be constantly pulled to GND
//!   PA1 = Display Reset^ - this could be connected to the 5v with a resistor and to the Gnd with a capacitor
//!   B12 = Display Backlight^ (with a PNP transistor) - use open drain output!
//!
//! PA11, PA12 = CAN RX, TX
//!
//! Heat request signal (open collector NPN transistor) on B11
//!
//! The remote changes the default config, the state displayed on the rgb led.
//! Controls the floor heating accordig to the config.
//!
//#![deny(unsafe_code)]
//#![deny(warnings)]
#![no_main]
#![no_std]

extern crate cortex_m;
#[macro_use]
extern crate cortex_m_rt as rt;
extern crate cortex_m_semihosting as sh;
extern crate embedded_hal;
extern crate lcd_hal;
extern crate nb;
extern crate onewire;
extern crate panic_halt;
extern crate room_pill;
extern crate stm32f1xx_hal as hal;

use crate::hal::{delay::Delay, prelude::*, stm32f1xx, watchdog::IndependentWatchdog};
use crate::rt::ExceptionFrame;
use embedded_hal::watchdog::{Watchdog, WatchdogEnable};
use ir::NecReceiver;
use lcd_hal::{hx1230, hx1230::Hx1230};
use onewire::{ds18x20::*, temperature::Temperature, *};
use room_pill::{
    display::*,
    ir,
    ir_remote::*,
    rgb::*,
    time::{Duration, Ticker, SysTicks, Seconds Time},
};
use rt::entry;

const MAX_THERMOMETER_COUNT: usize = 1; //max number of thermometers

struct Model {
    temperatures: [Option<Temperature>; MAX_THERMOMETER_COUNT],
    target_temperature: Temperature,
}

impl Model {
    fn new() -> Self {
        Self {
            temperatures: [None; MAX_THERMOMETER_COUNT],
            target_temperature: Temperature::from_celsius(80, 0),
        }
    }

    //update by IR remote
    fn ir_remote_command(&mut self, command: IrCommands) {
        match command {
            IrCommands::Right | IrCommands::Up => {
                self.target_temperature = self.target_temperature + Temperature::from_celsius(1, 0);
            }
            IrCommands::Left | IrCommands::Down => {
                self.target_temperature = self.target_temperature - Temperature::from_celsius(1, 0);
            }
            _ => {}
        }
    }

    //update by temp sensors
    fn update_temperature(&mut self, index: usize, temperature: Option<Temperature>) {
        self.temperatures[index] = temperature;
    }

    fn refresh_display<D: lcd_hal::Display>(
        &self,
        display: &mut D,
        rgb: &mut room_pill::rgb::RgbLed<
            hal::gpio::gpiob::PB13<hal::gpio::Output<hal::gpio::OpenDrain>>,
            hal::gpio::gpiob::PB14<hal::gpio::Output<hal::gpio::OpenDrain>>,
            hal::gpio::gpiob::PB15<hal::gpio::Output<hal::gpio::OpenDrain>>,
        >,
    ) {
        display.clear();

        display.set_position(0, 1);
        display.print(b"Cel  ");
        display.print(fmt_temp(self.target_temperature));

        if let Some(temp) = self.temperatures[0 as usize] {
            display.set_position(0, 2);
            display.print(b"Temp ");
            display.print(fmt_temp(temp));

            rgb.color(if temp > self.target_temperature {
                Colors::Red
            } else if temp < self.target_temperature {
                Colors::Blue
            } else {
                Colors::Green
            });
        } else {
            rgb.color(Colors::Black);
        }
    }
}

#[entry]
fn main() -> ! {
    let dp = stm32f1xx::Peripherals::take().unwrap();

    let mut watchdog = IndependentWatchdog::new(dp.IWDG);
    watchdog.start(2_000_000u32.us());

    let mut flash = dp.FLASH.constrain();

    let mut rcc = dp.RCC.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(72.mhz())
        .hclk(72.mhz())
        .pclk1(36.mhz())
        .pclk2(72.mhz())
        .freeze(&mut flash.acr);
    watchdog.feed();

    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);
    // Disables the JTAG to free up pb3, pb4 and pa15 for normal use
    afio.mapr.disable_jtag();

    //let mut hstdout = hio::hstdout().unwrap();
    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
    let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);

    // IR receiver^
    let ir_receiver = gpioa.pa15.into_pull_up_input(&mut gpioa.crh);

    // RGB led:
    let mut rgb = RgbLed::new(
        gpiob.pb13.into_open_drain_output(&mut gpiob.crh),
        gpiob.pb14.into_open_drain_output(&mut gpiob.crh),
        gpiob.pb15.into_open_drain_output(&mut gpiob.crh),
    );

    // on board led^:
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    // setup the lcd display:
    let mut rst = gpioa.pa7.into_push_pull_output(&mut gpioa.crl); //Display Reset^
    let cs = gpioa.pa6.into_push_pull_output(&mut gpioa.crl); // Display ChipSelect^
                                                              // let dc = gpioa.pa5.into_push_pull_output(&mut gpioa.crl); // Data / Command^
    let mosi = gpioa.pa4.into_push_pull_output(&mut gpioa.crl); // Data in
    let sck = gpioa.pa3.into_push_pull_output(&mut gpioa.crl); // Clock
    gpioa.pa2.into_floating_input(&mut gpioa.crl); //placeholder for Vcc, wired on board
    gpioa.pa1.into_floating_input(&mut gpioa.crl); //placeholder for Backlight, wired on board
    gpioa.pa0.into_floating_input(&mut gpioa.crl); //placeholder for Gnd, wired on board

    let cp = cortex_m::Peripherals::take().unwrap();
    let mut delay = Delay::new(cp.SYST, clocks);

    let mut display = hx1230::gpio::Hx1230Gpio::new(sck, mosi, cs, &mut rst, &mut delay);
    display.init();
    display.set_contrast(7);

    watchdog.feed();

    // setup the one wire thermometers:
    let io = gpiob.pb4.into_open_drain_output(&mut gpiob.crl);
    let mut one_wire = OneWirePort::new(io, delay);

    watchdog.feed();

    let ticker = Ticker::new(cp.DWT, cp.DCB, clocks);
    let mut receiver = ir::IrReceiver::<Time<u32, SysTicks>>::new();

    watchdog.feed();

    let mut model = Model::new();

    watchdog.feed();

    //store the addresses of temp sensors, start measurement on each:
    let mut roms = [[0u8; 8]; MAX_THERMOMETER_COUNT];
    let mut count = 0;

    let mut it = RomIterator::new(0);

    loop {
        watchdog.feed();

        match one_wire.iterate_next(true, &mut it) {
            Ok(None) => {
                break; //no or no more devices found -> stop
            }

            Ok(Some(rom)) => {
                if let Some(_device_type) = detect_18x20_devices(rom[0]) {
                    //writeln!(hstdout, "rom: {:?}", &rom).unwrap();
                    roms[count] = *rom;
                    count = count + 1;
                    let _ = one_wire.start_temperature_measurement(&rom);
                    if count >= MAX_THERMOMETER_COUNT {
                        break;
                    }
                }
                continue;
            }

            Err(_e) => {
                rgb.color(Colors::White);
                break;
            }
        }
    }

    //not mutable anymore
    let roms = roms;
    let count = count;

    let mut last_time = ticker.now();

    loop {
        watchdog.feed();

        //update the IR receiver statemachine:
        let ir_cmd = receiver.receive(ticker.now(), ir_receiver.is_low());

        match ir_cmd {
            Ok(ir::NecContent::Repeat) => {}
            Ok(ir::NecContent::Data(data)) => {
                let command = translate(data);
                model.ir_remote_command(command);
                model.refresh_display(&mut display, &mut rgb);
            }
            _ => {}
        }

        // calculate the time since last execution:
        let delta = ticker.now() - last_time;

        // do not execute the followings too often: (temperature conversion time of the sensors is a lower limit)
        if delta < 1u32.s() {
            continue;
        }

        led.toggle();

        // decrease the time resolution
        let delta_time = Duration::<u32, Seconds>::from(delta);

        // keep the difference measurement accurate by keeping the fractions...
        last_time = last_time + Duration::<u32, SysTicks>::from(delta_time);

        //read sensors and restart temperature measurement
        for i in 0..count {
            model.update_temperature(
                i,
                match one_wire.read_temperature_measurement_result(&roms[i]) {
                    Ok(temperature) => Some(temperature),
                    Err(_code) => None,
                },
            );
            let _ = one_wire.start_temperature_measurement(&roms[i]);
        }

        model.refresh_display(&mut display, &mut rgb);
    }
}

#[exception]
fn HardFault(_ef: &ExceptionFrame) -> ! {
    loop {}
    //panic!("HardFault at {:#?}", ef); //removed due to large code size
}

#[exception]
fn DefaultHandler(_irqn: i16) {
    //panic!("Unhandled exception (IRQn = {})", irqn);  //removed due to large code size
}
