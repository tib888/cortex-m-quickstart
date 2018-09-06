//! Read the temperature from DS18B20 1-wire temperature sensors connected to B4 GPIO
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
use hal::prelude::*;
use hal::stm32f103xx;
use ir::NecReceiver;
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

    //IR receiver^
    let ir_receiver = gpioa.pa15.into_pull_up_input(&mut gpioa.crh);

    //RGB led:
    let mut rgb = RgbLed::new(
        gpiob.pb13.into_push_pull_output(&mut gpiob.crh),
        gpiob.pb14.into_push_pull_output(&mut gpiob.crh),
        gpiob.pb15.into_push_pull_output(&mut gpiob.crh),
    );

    //On board led^:
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    //Thermostat opener SSR
    let mut thermostat = gpiob.pb6.into_push_pull_output(&mut gpiob.crl);

    //Pump starter SSR
    let mut pump = gpiob.pb7.into_push_pull_output(&mut gpiob.crl);

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

    //TODO: read sensors
    let floor_heating_sensors = floor_heating::Sensors {
        forward_temperature: None,
        return_temperature: None,
        floor_temperature: None,
        air_temperature: None,
    };

    loop {
        let t = unsafe { TICK };
        let ir_cmd = receiver.receive(t, ir_receiver.is_low());

        match ir_cmd {
            Ok(ir::NecContent::Repeat) => {}
            Ok(ir::NecContent::Data(data)) => match data >> 8 {
                0x20F04E | 0x807FC2 => rgb.color(Colors::Red),
                0x20F08E | 0x807FF0 => rgb.color(Colors::Green),
                0x20F0C6 | 0x807F08 => rgb.color(Colors::Yellow),
                0x20F086 | 0x807F18 => rgb.color(Colors::Blue),
                0x20F022 | 0x807FC8 => rgb.color(Colors::White),
                _ => {
                    led.toggle();
                    rgb.color(Colors::Black)
                }
            },
            _ => {}
        }

        //TODO: feed the watchdog, calculate the time since last execution,
        let delta_time = 1u32;

        floor_heating_state =
            floor_heating_state.update(&floor_heating_config, &floor_heating_sensors, delta_time);

        //drive outputs, send messages:
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
