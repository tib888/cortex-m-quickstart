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
//! The remote changes the default config, the state displayed on the rgb led.
//! Cotrols the floor heating accordig to the config.
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
extern crate ir;
extern crate nb;
extern crate onewire;
extern crate panic_semihosting;
extern crate pcd8544_hal;
extern crate room_pill;
extern crate stm32f103xx_hal as hal;

//use core::fmt::Write;
use embedded_hal::spi;
use hal::delay::Delay;
use hal::prelude::*;
use hal::spi::Spi;
use hal::stm32f103xx;
use ir::NecReceiver;
use onewire::ds18x20::*;
use onewire::temperature::Temperature;
use onewire::*;
use pcd8544_hal::{Pcd8544, Pcd8544Spi};
use room_pill::floor_heating;
use room_pill::pump::*;
use room_pill::rgb::*;
use room_pill::time::*;
use room_pill::valve::*;
use rt::ExceptionFrame;
//use sh::hio;

entry!(main);

fn print_temp<T: Pcd8544>(display: &mut T, row: u8, prefix: &str, temp: &Option<Temperature>) {
    display.set_position(0, row);
    display.print(prefix);

    if let Some(temp) = temp {
        let t = temp.whole_degrees();
        display.print_char(if t < 0 { '-' } else { ' ' } as u8);

        let t: u8 = t.abs() as u8;
        if t > 9 {
            display.print_char('0' as u8 + (t / 10));
        }
        display.print_char('0' as u8 + (t % 10));
        display.print_char('.' as u8);

        //round fraction to one digit:
        // 0	0.000
        // 1	0.063
        // 2	0.125
        // 3	0.188
        // 4	0.250
        // 5	0.313
        // 6	0.375
        // 7	0.438
        // 8	0.500
        // 9	0.563
        // 10	0.625
        // 11	0.688
        // 12	0.750
        // 13	0.813
        // 14	0.875
        // 15	0.938
        static ROUND_TABLE: &[u8] = b"0112334456678899";
        display.print_char(ROUND_TABLE[temp.fraction_degrees() as usize]);
    } else {
        display.print("-----");
    }
}

fn main() -> ! {
    //let mut hstdout = hio::hstdout().unwrap();

    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32f103xx::Peripherals::take().unwrap();

    let mut rcc = dp.RCC.constrain();

    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
    let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);

    // IR receiver^
    let ir_receiver = gpioa.pa15.into_pull_up_input(&mut gpioa.crh);

    // RGB led:
    let mut rgb = RgbLed::new(
        gpiob.pb13.into_push_pull_output(&mut gpiob.crh),
        gpiob.pb14.into_push_pull_output(&mut gpiob.crh),
        gpiob.pb15.into_push_pull_output(&mut gpiob.crh),
    );

    // on board led^:
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    // valve opener SSR^
    let mut valve = ValveSSR::new(gpiob.pb6.into_open_drain_output(&mut gpiob.crl));

    // pump starter SSR^
    let mut pump = PumpSSR::new(gpiob.pb7.into_open_drain_output(&mut gpiob.crl));

    let mut flash = dp.FLASH.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);

    // setup SPI for the PCD8544 display:
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl); //PA5 = Display SPI clock
    let miso = gpioa.pa6.into_floating_input(&mut gpioa.crl); //PA6 = Display SPI input - not used
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl); //PA7 = Display SPI data
    let spi_mode = spi::Mode {
        phase: spi::Phase::CaptureOnFirstTransition,
        polarity: spi::Polarity::IdleLow,
    };

    let spi = Spi::spi1(
        dp.SPI1,
        (sck, miso, mosi),
        &mut afio.mapr,
        spi_mode,
        4.mhz(),
        clocks,
        &mut rcc.apb2,
    );

    // other pins for PCD8544
    let dc = gpioa.pa4.into_push_pull_output(&mut gpioa.crl); // PA4 = Display Data/Command
    let cs = gpioa.pa3.into_push_pull_output(&mut gpioa.crl); // PA3 = Display Chip Select
    let mut rst = gpioa.pa1.into_push_pull_output(&mut gpioa.crl); // PA1 = Display Reset

    let mut delay = Delay::new(cp.SYST, clocks);
    let mut display = Pcd8544Spi::new(spi, dc, cs, &mut rst, &mut delay);
    display.init();

    // setup the one wire thermometers:
    // free PB3, PB4 from JTAG to be used as GPIO:
    afio.mapr
        .mapr()
        .modify(|_, w| unsafe { w.swj_cfg().bits(1) });
    let io = gpiob.pb4.into_open_drain_output(&mut gpiob.crl);
    let mut one_wire = OneWirePort::new(io, delay);

    let tick = Ticker::new(cp.DWT, cp.DCB, clocks);

    let mut receiver = ir::IrReceiver::<Time<Ticks>>::new();

    let mut floor_heating_state = floor_heating::State::Standby(Duration::sec(0));

    //this config will be updated by IR remote or CAN messages
    let mut floor_heating_config = floor_heating::Config {
        max_forward_temperature: Temperature::from_celsius(40, 0),
        max_floor_temperature: Temperature::from_celsius(29, 0),
        target_air_temperature: None, //Some(Temperature::from_celsius(19, 0)),
        temperature_histeresis: Temperature::from_celsius(0, 1),
        freeze_protection: floor_heating::FreezeProtectionConfig {
            min_temperature: Temperature::from_celsius(5, 0),
            safe_temperature: Temperature::from_celsius(8, 0),
            check_interval: Duration::<Seconds>::hms(4, 0, 0), //4 hour
            check_duration: Duration::<Seconds>::hms(0, 4, 0), //4 min
        },
        after_circulation_duration: Duration::<Seconds>::hms(0, 4, 0),
    };

    let mut floor_heating_sensors = floor_heating::Sensors {
        forward_temperature: None,
        return_temperature: None,
        air_temperature: None,
        floor_temperature: None,
    };

    //store the addresses of temp sensors, start measurement on each:
    const MAX_COUNT: usize = 4;
    let mut roms = [[0u8; 8]; MAX_COUNT];
    let mut count = 0;

    let mut it = RomIterator::new(0);
    loop {
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
                    if count >= MAX_COUNT {
                        break;
                    }
                }
                continue;
            }

            Err(_e) => {
                rgb.color(Colors::Cyan);
            }
        }
    }

    //not mutable anymore
    let roms = roms;
    let count = count;

    let mut last_time = tick.now();

    loop {
        //TODO: feed the watchdog
        let ir_cmd = receiver.receive(tick.now(), ir_receiver.is_low());

        match ir_cmd {
            Ok(ir::NecContent::Repeat) => {}
            Ok(ir::NecContent::Data(data)) => {
                floor_heating_config.target_air_temperature = match data >> 8 {
                    0x807F80 => Some(Temperature::from_celsius(20, 0)), //0
                    0x807F72 => Some(Temperature::from_celsius(21, 0)), //1
                    0x807FB0 => Some(Temperature::from_celsius(22, 0)), //2
                    0x807F30 => Some(Temperature::from_celsius(23, 0)), //3
                    0x807F52 => Some(Temperature::from_celsius(24, 0)), //4
                    0x807F90 => Some(Temperature::from_celsius(15, 0)), //5
                    0x807F10 => Some(Temperature::from_celsius(16, 0)), //6
                    0x807F62 => Some(Temperature::from_celsius(17, 0)), //7
                    0x807FA0 => Some(Temperature::from_celsius(18, 0)), //8
                    0x807F20 => Some(Temperature::from_celsius(19, 0)), //9
                    0x20F002 | 0x807F68 => floor_heating_config
                        .target_air_temperature
                        .map(|t| t + Temperature::from_celsius(0, 4)), //up +4/16 C
                    0x20F082 | 0x807F58 => floor_heating_config
                        .target_air_temperature
                        .map(|t| t - Temperature::from_celsius(0, 4)), //down -4/16 C
                    0x20F04E | 0x807FC2 => Some(Temperature::from_celsius(22, 0)), //red
                    0x20F08E | 0x807FF0 => Some(Temperature::from_celsius(20, 0)), //green
                    0x20F0C6 | 0x807F08 => Some(Temperature::from_celsius(18, 0)), //yellow
                    0x20F086 | 0x807F18 => Some(Temperature::from_celsius(15, 0)), //blue
                    0x20F022 | 0x807FC8 => None,                        //OK
                    _ => floor_heating_config.target_air_temperature,   //etc.
                };
                rgb.color(Colors::Black);
                // if let Some(temp) = floor_heating_config.target_air_temperature {
                //     writeln!(hstdout, "target = {:?}/16 C", temp).unwrap();
                //     print_temp(display, temp);
                // } else {
                //     writeln!(hstdout, "target = none").unwrap();
                // }
                print_temp(
                    &mut display,
                    5,
                    "Cel:    ",
                    &floor_heating_config.target_air_temperature,
                );
            }
            _ => {}
        }

        // calculate the time since last execution:
        let delta = tick.now() - last_time;

        // do not execute the followings too often: (temperature conversion time of the sensors is a lower limit)
        if delta.count < tick.frequency {
            continue;
        }

        led.toggle();

        // decrease the time resolution
        let delta_time = Duration::sec(delta.count / tick.frequency);

        // keep the difference measurement is accurate...
        last_time = last_time + Duration::<Ticks>::from(delta_time.count * tick.frequency);

        //read sensors and restart temperature measurement
        for i in 0..count {
            let result = match one_wire.read_temperature_measurement_result(&roms[i]) {
                Ok(temperature) => {
                    //writeln!(hstdout, "temp[{}] = {:?}/16 C", i, temperature).unwrap();
                    Some(temperature)
                }
                Err(_code) => None,
            };
            match i {
                0 => floor_heating_sensors.forward_temperature = result,
                1 => floor_heating_sensors.return_temperature = result,
                2 => floor_heating_sensors.air_temperature = result,
                3 => floor_heating_sensors.floor_temperature = result,
                _ => assert!(false),
            };
            let _ = one_wire.start_temperature_measurement(&roms[i]);
        }

        floor_heating_state =
            floor_heating_state.update(&floor_heating_config, &floor_heating_sensors, delta_time);

        //writeln!(hstdout, "{:?}", floor_heating_state).unwrap();

        // drive outputs, send messages:
        display.clear();
        display.set_position(0, 0);
        match floor_heating_state {
            floor_heating::State::Heating(defreeze) => {
                rgb.color(if defreeze {
                    // display.print("Olvasztas");
                    Colors::Purple
                } else {
                    // display.print("Futes");
                    Colors::Red
                });
                valve.open();
                pump.start();
                //CAN: heat request
            }
            floor_heating::State::AfterCirculation(_) => {
                // display.print("Utokeringetes");
                rgb.color(Colors::Yellow);
                valve.close();
                pump.start();
                //CAN: no heat request
            }
            floor_heating::State::Standby(_) => {
                // display.print("Keszenlet");
                rgb.color(Colors::Green);
                valve.close();
                pump.stop();
                //CAN: no heat request
            }
            floor_heating::State::FreezeProtectionCheckCirculation(_) => {
                // display.print("Fagyvizsgalat");
                rgb.color(Colors::Blue);
                valve.close();
                pump.start();
                //CAN: no heat request
            }
            floor_heating::State::Error => {
                // display.print("Szenzorhiba");
                rgb.color(Colors::White);
                //CAN: sensor missing error
            }
        };

        print_temp(
            &mut display,
            1,
            "Elore:  ",
            &floor_heating_sensors.forward_temperature,
        );
        print_temp(
            &mut display,
            2,
            "Vissza: ",
            &floor_heating_sensors.return_temperature,
        );
        print_temp(
            &mut display,
            3,
            "Padlo:  ",
            &floor_heating_sensors.floor_temperature,
        );
        print_temp(
            &mut display,
            4,
            "Levego: ",
            &floor_heating_sensors.air_temperature,
        );
        print_temp(
            &mut display,
            5,
            "Cel:    ",
            &floor_heating_config.target_air_temperature,
        );
    }

    //writeln!(hstdout, "end").unwrap();
}

exception!(HardFault, hard_fault);

fn hard_fault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault at {:#?}", ef);
}

exception!(*, default_handler);

fn default_handler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}
