//! Read the NEC IR remote commands on A15 GPIO as input with internal pullup
//! RGB led on PB13, PB14, PB15 as push pull output
//! Reacts to the colored buttons on the remotes with colors on the rgb led.
//#![deny(unsafe_code)]
//#![deny(warnings)]
#![no_main]
#![no_std]

use panic_halt as _;

use cortex_m;
#[macro_use]
use cortex_m_rt;
use embedded_hal;
use panic_halt;
use room_pill;
use stm32f1xx_hal;

use embedded_hal::{
	digital::v2::{InputPin},
};
use cortex_m_rt::{entry, exception, ExceptionFrame};
use stm32f1xx_hal::{ prelude::* };
use room_pill::{
	ir,
	ir::NecReceiver,
    timing::{Ticker},
    rgb::{Colors, RgbLed, Rgb},
};

#[entry]
fn main() -> ! {
    let core = cortex_m::Peripherals::take().unwrap();
    let device = stm32f1xx_hal::pac::Peripherals::take().unwrap();
    let mut rcc = device.RCC.constrain();
    let mut gpioa = device.GPIOA.split(&mut rcc.apb2);
	let mut gpiob = device.GPIOB.split(&mut rcc.apb2);
    let mut gpioc = device.GPIOC.split(&mut rcc.apb2);
    
    let mut afio = device.AFIO.constrain(&mut rcc.apb2);

    // Disables the JTAG to free up pb3, pb4 and pa15 for normal use
    let (pa15, _pb3_itm_swo, _pb4) = afio.mapr.disable_jtag(gpioa.pa15, gpiob.pb3, gpiob.pb4);

    //IR receiver^
    let ir_receiver = pa15.into_pull_up_input(&mut gpioa.crh);

    //RGB led:
    let mut rgb = RgbLed::new(
        gpiob.pb13.into_push_pull_output(&mut gpiob.crh),
        gpiob.pb14.into_push_pull_output(&mut gpiob.crh),
        gpiob.pb15.into_push_pull_output(&mut gpiob.crh),
    );

    //On board led^:
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    let mut flash = device.FLASH.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let tick = Ticker::new(core.DWT, core.DCB, clocks);

    let mut receiver = ir::IrReceiver::new(); // period = 0.5ms = 500us

    let mut color = Colors::White as u32;

    loop {
        let now = tick.now();
        let ir_cmd = receiver.receive(ir_receiver.is_low().unwrap(), now, |last| {
			tick.to_us(now - last).into()
		});

        let c = match ir_cmd {
            Ok(ir::NecContent::Repeat) => None,
            Ok(ir::NecContent::Data(data)) => match data >> 8 {
                0x20F04E | 0x807FC2 => Some(Colors::Red as u32),
                0x20F08E | 0x807FF0 => Some(Colors::Green as u32),
                0x20F0C6 | 0x807F08 => Some(Colors::Yellow as u32),
                0x20F086 | 0x807F18 => Some(Colors::Blue as u32),
                0x20F022 | 0x807FC8 => Some(Colors::White as u32),
                _ => {
                    led.toggle().unwrap();
                    Some(Colors::Black as u32)
                }
            },
            _ => None,
        };

        if let Some(c) = c {
            if led.is_set_high().unwrap() {
                //mix mode
                color = color ^ c;
            } else {
                //set mode
                color = c;
            }

            rgb.raw_color(color).unwrap();
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
