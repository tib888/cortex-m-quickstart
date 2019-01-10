//! Read the NEC IR remote commands on A15 GPIO as input with internal pullup
//! RGB led on PB13, PB14, PB15 as push pull output
//! Reacts to the colored buttons on the remotes with colors on the rgb led.
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
extern crate onewire;
extern crate panic_semihosting;
extern crate room_pill;
extern crate stm32f103xx_hal as hal;

use crate::hal::prelude::*;
use crate::hal::stm32f103xx;
use crate::hal::time::*;
use crate::rt::entry;
use crate::rt::ExceptionFrame;
use ir::NecReceiver;
use room_pill::rgb::*;

#[derive(Copy, Clone)]
struct Time {
    now: hal::time::Instant,
    freq: u32,
}

impl Time {
    fn new(tick: &hal::time::MonoTimer) -> Time {
        Time {
            now: tick.now(),
            freq: tick.frequency().0,
        }
    }
}

impl ir::Instant for Time {
    /// called on an older instant, returns the elapsed microseconds until the given now
    fn elapsed_us_till(&self, now: &Self) -> u32 {
        self.now.elapsed_till(&now.now) * 1_000_000u32 / self.freq
    }
}

#[entry]
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

    let mut flash = dp.FLASH.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let trace_enabled = enable_trace(cp.DCB);
    let tick = MonoTimer::new(cp.DWT, trace_enabled, clocks);

    let mut receiver = ir::IrReceiver::<Time>::new(); // period = 0.5ms = 500us

    let mut color = Colors::White as u32;

    loop {
        let t = Time::new(&tick);
        let ir_cmd = receiver.receive(t, ir_receiver.is_low());

        let c = match ir_cmd {
            Ok(ir::NecContent::Repeat) => None,
            Ok(ir::NecContent::Data(data)) => match data >> 8 {
                0x20F04E | 0x807FC2 => Some(Colors::Red as u32),
                0x20F08E | 0x807FF0 => Some(Colors::Green as u32),
                0x20F0C6 | 0x807F08 => Some(Colors::Yellow as u32),
                0x20F086 | 0x807F18 => Some(Colors::Blue as u32),
                0x20F022 | 0x807FC8 => Some(Colors::White as u32),
                _ => {
                    led.toggle();
                    Some(Colors::Black as u32)
                }
            },
            _ => None,
        };

        if let Some(c) = c {
            if led.is_set_high() {
                //mix mode
                color = color ^ c;
            } else {
                //set mode
                color = c;
            }

            rgb.raw_color(color);
        }
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
