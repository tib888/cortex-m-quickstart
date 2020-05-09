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

use panic_halt as _;

use cortex_m;
use cortex_m_rt;
use embedded_hal;
use room_pill;
use stm32f1xx_hal;

use cortex_m_rt::{entry, exception, ExceptionFrame};
use embedded_hal::digital::v2::InputPin;
use lcd_hal::{hx1230, hx1230::Hx1230, Display};
use onewire::{ds18x20::*, temperature::Temperature, *};
use room_pill::{
    display::*,
    ir,
    ir::NecReceiver,
    ir_remote::*,
    rgb::{Colors, Rgb, RgbLed},
    timing::Ticker,
};
use stm32f1xx_hal::{delay::Delay, prelude::*, watchdog::IndependentWatchdog};

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

    fn refresh_display<D: lcd_hal::Display, RGB: Rgb>(
        &self,
        display: &mut D,
        rgb: &mut RGB,
    ) -> Result<(), D::Error> {
        display.clear()?;

        display.set_position(0, 1)?;
        display.print(b"Cel  ")?;
        display.print(unsafe { fmt_temp(self.target_temperature) })?;

        if let Some(temp) = self.temperatures[0 as usize] {
            display.set_position(0, 2)?;
            display.print(b"Temp ")?;
            display.print(unsafe { fmt_temp(temp) })?;

            let _ = rgb.color(if temp > self.target_temperature {
                Colors::Red
            } else if temp < self.target_temperature {
                Colors::Blue
            } else {
                Colors::Green
            });
        } else {
            let _ = rgb.color(Colors::Black);
        }

        Ok(())
    }
}

#[entry]
fn main() -> ! {
    let device = stm32f1xx_hal::pac::Peripherals::take().unwrap();
    let mut watchdog = IndependentWatchdog::new(device.IWDG);
    watchdog.start(stm32f1xx_hal::time::U32Ext::ms(2_000u32));

    let mut flash = device.FLASH.constrain();

    let mut rcc = device.RCC.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(72.mhz())
        .hclk(72.mhz())
        .pclk1(36.mhz())
        .pclk2(72.mhz())
        .adcclk(9.mhz()) //ADC clock: PCLK2 / 8. User specified value is be approximated using supported prescaler values 2/4/6/8.
        .freeze(&mut flash.acr);
    watchdog.feed();

    let mut afio = device.AFIO.constrain(&mut rcc.apb2);

    //configure pins:
    let mut gpioa = device.GPIOA.split(&mut rcc.apb2);
    let mut gpiob = device.GPIOB.split(&mut rcc.apb2);
    let mut gpioc = device.GPIOC.split(&mut rcc.apb2);

    // Disables the JTAG to free up pb3, pb4 and pa15 for normal use
    let (pa15, _pb3_itm_swo, pb4) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);
    //let mut hstdout = hio::hstdout().unwrap();

    // IR receiver^
    let ir_receiver = pa15.into_pull_up_input(&mut gpioa.crh);

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

    let core = cortex_m::Peripherals::take().unwrap();
    let mut delay = Delay::new(core.SYST, clocks);

    let mut display = hx1230::gpio::Hx1230Gpio::new(sck, mosi, cs, &mut rst, &mut delay).unwrap();
    display.init().unwrap();
    display.set_contrast(7).unwrap();
    display.clear().unwrap();

    watchdog.feed();

    // setup the one wire thermometers:
    let mut one_wire = {
        // DS18B20 1-wire temperature sensors connected to B4 GPIO
        let onewire_io = pb4.into_open_drain_output(&mut gpiob.crl);
        OneWirePort::new(onewire_io, delay).unwrap()
    };

    watchdog.feed();

    let tick = Ticker::new(core.DWT, core.DCB, clocks);
    let mut receiver = ir::IrReceiver::new();

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
                rgb.color(Colors::White).unwrap();
                break;
            }
        }
    }

    //not mutable anymore
    let roms = roms;
    let count = count;

    let mut last_big = tick.now();

    loop {
        watchdog.feed();

        let now = tick.now();

        //update the IR receiver statemachine:
        let ir_cmd = receiver.receive(ir_receiver.is_low().unwrap(), now, |last| {
            tick.to_us(now - last).into()
        });

        match ir_cmd {
            Ok(ir::NecContent::Repeat) => {}
            Ok(ir::NecContent::Data(data)) => {
                let command = translate(data);
                model.ir_remote_command(command);
                model.refresh_display(&mut display, &mut rgb).unwrap();
            }
            _ => {}
        }
        // do not execute the followings too often: (temperature conversion time of the sensors is a lower limit)
        if u32::from(now - last_big) < tick.frequency {
            continue;
        }

        last_big = now;

        led.toggle().unwrap();

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

        model.refresh_display(&mut display, &mut rgb).unwrap();
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
