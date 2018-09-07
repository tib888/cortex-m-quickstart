//! DS18B20 1-wire temperature sensors connected to B4 GPIO
//! JTAG is removed from B3, B4 to make it work
//!
//! Read the NEC IR remote commands on A15 GPIO as input with internal pullup
//!
//! RGB led on PB13, PB14, PB15 as push pull output
//!
//! Solid state relay 1 connected to B6 drives the thermostat
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
extern crate room_pill;
extern crate stm32f103xx_hal as hal;

use cortex_m::peripheral::syst::SystClkSource;
use hal::delay::Delay;
use hal::prelude::*;
use hal::stm32f103xx;
use ir::NecReceiver;
use onewire::ds18x20::*;
use onewire::*;
use room_pill::floor_heating;
use room_pill::rgb::*;
use rt::ExceptionFrame;

static mut TICK: u32 = 0;

entry!(main);

fn main() -> ! {
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

    // thermostat opener SSR
    let mut thermostat = gpiob.pb6.into_push_pull_output(&mut gpiob.crl);

    // pump starter SSR
    let mut pump = gpiob.pb7.into_push_pull_output(&mut gpiob.crl);

    // setup the one wire thermometers:
    // free PB3, PB4 from JTAG to be used as GPIO:
    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);
    afio.mapr
        .mapr()
        .modify(|_, w| unsafe { w.swj_cfg().bits(1) });
    let mut flash = dp.FLASH.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    //let clocks = rcc.cfgr
    //    .sysclk(72.mhz())
    //     .pclk1(32.mhz())
    //    .freeze(&mut flash.acr);
    let delay = Delay::new(cp.SYST, clocks);
    let io = gpiob.pb4.into_open_drain_output(&mut gpiob.crl);
    let mut one_wire = OneWirePort::new(io, delay);

    // configures the system timer to trigger a SysTick exception every half millisecond
    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(4_000); // period = 500us
    syst.enable_counter();
    syst.enable_interrupt();

    let mut receiver = ir::IrReceiver::new(4_000 / 8); // period = 0.5ms = 500us

    let mut floor_heating_state = floor_heating::State::Standby(0u32);

    //TODO: update this config by IR remote
    let mut floor_heating_config = floor_heating::Config {
        max_forward_temperature: 40.0,
        max_floor_temperature: 29.0,
        target_air_temperature: Some(21.0),
        temperature_histeresis: 0.1,
        freeze_protection: floor_heating::FreezeProtectionConfig {
            min_temperature: 4.0,
            safe_temperature: 8.0,
            check_interval: 60 * 60 * 4, //4 hour
            check_duration: 60 * 4,      //4 min
        },
        after_circulation_duration: 240,
    };

    let mut floor_heating_sensors = floor_heating::Sensors {
        forward_temperature: None,
        return_temperature: None,
        air_temperature: None,
        floor_temperature: None,
    };

    //store the addresses of temp sensors, start measurement on each:
    const max_count: usize = 4;
    let mut roms = [[0u8; 8]; max_count];
    let mut count = 0;

    let mut it = RomIterator::new(0);
    loop {
        match one_wire.iterate_next(true, &mut it) {
            Ok(None) => {
                break; //no or no mode devices found -> stop
            }

            Ok(Some(rom)) => {
                if let Some(_device_type) = detect_18x20_devices(rom[0]) {
                    roms[count] = *rom;
                    count = count + 1;
                    one_wire.start_temperature_measurement(&rom);
                    if count >= max_count {
                        break;
                    }
                }
                continue;
            }

            Err(e) => {
                rgb.color(Colors::White);
                led.toggle();
            }

            _ => {
                led.toggle();
            }
        }
    }

    //not mutable anymore
    let roms = roms;
    let count = count;

    let mut last_time = None;

    loop {
        //TODO: feed the watchdog

        let t = unsafe { TICK };
        let ir_cmd = receiver.receive(t, ir_receiver.is_low());

        match ir_cmd {
            Ok(ir::NecContent::Repeat) => {}
            Ok(ir::NecContent::Data(data)) => match data >> 8 {
                0x20F04E | 0x807FC2 =>
                //red
                {
                    floor_heating_config.target_air_temperature = Some(22.0);
                    led.toggle();
                }
                0x20F08E | 0x807FF0 =>
                //green
                {
                    floor_heating_config.target_air_temperature = Some(20.0);
                    led.toggle();
                }
                0x20F0C6 | 0x807F08 =>
                //yellow
                {
                    floor_heating_config.target_air_temperature = Some(18.0);
                    led.toggle();
                }
                0x20F086 | 0x807F18 =>
                //blue
                {
                    floor_heating_config.target_air_temperature = Some(15.0);
                    led.toggle();
                }
                0x20F022 | 0x807FC8 =>
                //center
                {
                    floor_heating_config.target_air_temperature = None;
                    led.toggle();
                }
                _ => {}
            },
            _ => {}
        }

        // calculate the time since last execution:
        if let Some(last_t) = last_time {
            let delta_time = t - last_t;

            const div: u32 = 11; //2048*0.5ms s approx 1sec

            // do not execute the followings to often: (teperature conversion time of the sensors is a lower limit)
            if delta_time < (1 << div) {
                continue;
            }

            // decrease the time resolution
            delta_time = delta_time >> div;

            // but the difference measurement is accurate
            last_time = Some(last_t + (delta_time << div));

            //read sensors and restart measurement
            for i in 0..count {
                let result = match one_wire.read_temperature_measurement_result(&roms[i]) {
                    Ok(temperature) => Some(temperature),
                    Err(code) => None,
                };
                match i {
                    0 => floor_heating_sensors.forward_temperature = result,
                    1 => floor_heating_sensors.return_temperature = result,
                    2 => floor_heating_sensors.air_temperature = result,
                    3 => floor_heating_sensors.floor_temperature = result,
                };
                one_wire.start_temperature_measurement(&roms[i]);
            }

            floor_heating_state = floor_heating_state.update(
                &floor_heating_config,
                &floor_heating_sensors,
                delta_time,
            );

            // drive outputs, send messages:
            match floor_heating_state {
                floor_heating::State::Heating(defreeze) => {
                    rgb.color(if defreeze {
                        Colors::Purple
                    } else {
                        Colors::Red
                    });
                    thermostat.set_high();
                    pump.set_high();
                    //CAN: heat request
                }
                floor_heating::State::AfterCirculation(_) => {
                    rgb.color(Colors::Yellow);
                    thermostat.set_low();
                    pump.set_high();
                    //CAN: no heat request
                }
                floor_heating::State::Standby(_) => {
                    rgb.color(Colors::Green);
                    thermostat.set_low();
                    pump.set_low();
                    //CAN: no heat request
                }
                floor_heating::State::FreezeProtectionCheckCirculation(_) => {
                    rgb.color(Colors::Blue);
                    thermostat.set_low();
                    pump.set_high();
                    //CAN: no heat request
                }
                floor_heating::State::Error => {
                    rgb.color(Colors::White);
                    //CAN: sensor missing error
                }
            }
        } else {
            last_time = Some(t);
        }
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
