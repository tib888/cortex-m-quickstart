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
extern crate nb;
extern crate onewire;
extern crate panic_semihosting;
extern crate room_pill;
extern crate stm32f103xx_hal as hal;

use cortex_m::peripheral::syst::SystClkSource;
use hal::prelude::*;
use hal::stm32f103xx;
use ir::NecReceiver;
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

    // configures the system timer to trigger a SysTick exception every half millisecond
    let mut syst = cp.SYST;
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(4_000); // period = 500us
    syst.enable_counter();
    syst.enable_interrupt();

    let mut receiver = ir::IrReceiver::new(4_000 / 8); // period = 0.5ms = 500us

    let mut color = Colors::White as u32;

    loop {
        let t = unsafe { TICK };
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

            rgb.color(color);
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
