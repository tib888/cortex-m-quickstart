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

use crate::hal::prelude::*;
use crate::hal::stm32f103xx;
use crate::rt::entry;
use crate::rt::ExceptionFrame;
use ir::NecReceiver;
use room_pill::rgb::{Colors, RgbLed};
use room_pill::time::{Ticker, Ticks, Time};
//use sh::hio;
//use core::fmt::Write;

#[entry]
fn main() -> ! {
	window_unit_main();
}

fn window_unit_main() -> ! {
	let cp = cortex_m::Peripherals::take().unwrap();
	let dp = stm32f103xx::Peripherals::take().unwrap();

	let mut rcc = dp.RCC.constrain();

	let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
	let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
	let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);

	//free PB3, PB4 from JTAG to be used as GPIO:
	let mut afio = dp.AFIO.constrain(&mut rcc.apb2);
	//#[allow(unused_unsafe)]
	afio.mapr
		.mapr()
		.modify(|_, w| unsafe { w.swj_cfg().bits(1) });

	//Window unit pins:
	let _valve_sense_a = gpioa.pa0.into_floating_input(&mut gpioa.crl);
	let _valve_sense_b = gpioa.pa1.into_floating_input(&mut gpioa.crl);
	let _roll_up_button = gpioa.pa2.into_pull_up_input(&mut gpioa.crl);
	let _roll_down_button = gpioa.pa3.into_pull_up_input(&mut gpioa.crl);
	let _motion_alarm = gpioa.pa4.into_pull_up_input(&mut gpioa.crl);
	let _wnd_open_alarm = gpioa.pa5.into_pull_up_input(&mut gpioa.crl);
	//let _address_pot = gpioa.pa6.into_anaglog_input(&mut gpioa.crl);
	//let _roll_hall = gpioa.pa6.into_anaglog_input(&mut gpioa.crl);

	//a11, a12: can

	//IR receiver^
	let ir_receiver = gpioa.pa15.into_pull_up_input(&mut gpioa.crh);

	//let mut _lux0 = gpiob.pb0.into_anaglog_input(&mut gpiob.crl);
	//let mut _lux1 = gpiob.pb1.into_anaglog_input(&mut gpiob.crl);

	//onewire temperature measurement:
	let _io = gpiob.pb4.into_open_drain_output(&mut gpiob.crl); //pb3, pb4 used as JTDO JTRST so they need to be freed somehow first!

	let mut _ir_led = gpiob.pb5.into_push_pull_output(&mut gpiob.crl);

	let mut _relay0 = gpiob.pb6.into_push_pull_output(&mut gpiob.crl);
	let mut _relay1 = gpiob.pb7.into_push_pull_output(&mut gpiob.crl);
	let mut _relay2 = gpiob.pb8.into_push_pull_output(&mut gpiob.crh);
	let mut _relay3 = gpiob.pb9.into_push_pull_output(&mut gpiob.crh);

	let mut _valve_drive_a = gpiob.pb10.into_push_pull_output(&mut gpiob.crh);
	let mut _valve_drive_b = gpiob.pb11.into_push_pull_output(&mut gpiob.crh);

	//RGB led:
	let mut rgb = RgbLed::new(
		gpiob.pb13.into_push_pull_output(&mut gpiob.crh),
		gpiob.pb14.into_push_pull_output(&mut gpiob.crh),
		gpiob.pb15.into_push_pull_output(&mut gpiob.crh),
	);

	//on board led^:
	let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

	let mut flash = dp.FLASH.constrain();
	let clocks = rcc.cfgr.freeze(&mut flash.acr);
	let tick = Ticker::new(cp.DWT, cp.DCB, clocks);

	// {
	// 	//APB2 clock enable (for external irqs)
	// 	let afio = dp.AFIO; ////TODO .constrain(&mut rcc.apb2);
	// 				  //rcc.apb2.enr().modify(|_, w| w.afioen().enabled());
	// 				  //rcc.apb2.rstr().modify(|_, w| w.afiorst().set_bit());
	// 				  //rcc.apb2.rstr().modify(|_, w| w.afiorst().clear_bit());

	// 	//index of GPIOA = 0 = bits; $i = 15;
	// 	//$i/4 + 1 = EXTICR4;
	// 	//$i % 4 = offset;
	// 	let bits = 0;
	// 	let offset = 15 & 0b11;
	// 	afio.exticr4
	// 		.modify(|r, w| unsafe { w.bits((r.bits() & !(0b1111 << offset)) | (bits << offset)) });

	// 	let exti = dp.EXTI; //TODO .constrain(...);

	// 	// configure EXTI0 interrupt			// FIXME turn this into a higher level API
	// 	exti.imr.write(|w| w.mr15().set_bit()); // unmask the interrupt (EXTI)
	// 	// dp.EXTI.emr.write(|w| w.mr15().set_bit()); // unmask the event (EXTI)
	// 	exti.rtsr.write(|w| w.tr15().set_bit()); // trigger interrupt on rising edge
	// 	exti.ftsr.write(|w| w.tr15().set_bit()); // trigger interrupt on falling edge

	// 	// trigger the irq from code:
	// 	//rtfm::set_pending(Interrupt::EXTI0);

	// 	let mut nvic = cp.NVIC;
	// 	nvic.enable(Interrupt::EXTI0);
	// 	// trigger the `EXTI0` interrupt
	// 	nvic.set_pending(Interrupt::EXTI0);
	// }

	rgb.color(Colors::Black);

	let mut receiver = ir::IrReceiver::<Time<Ticks>>::new();

	loop {
		let ir_cmd = receiver.receive(tick.now(), ir_receiver.is_low());

		match ir_cmd {
			Ok(ir::NecContent::Repeat) => {}
			Ok(ir::NecContent::Data(_data)) => {
				led.toggle();
			}
			_ => {}
		}
	}
}

// interrupt!(EXTI0, exti0, state: Option<HStdout> = None);

// fn exti0(_state: &mut Option<HStdout>) {
// 	if state.is_none() {
// 	    *state = Some(hio::hstdout().unwrap());
// 	}

// 	if let Some(hstdout) = state.as_mut() {
// 	    hstdout.write_str(">").unwrap();
// 	}

// 	while interrupt_pending() {
// 	}

// 	//clear the pending interrupt flag
// 	r.EXTI.pr.write(|w| w.pr15().set_bit());
// }

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
	panic!("HardFault at {:#?}", ef);
}

#[exception]
fn DefaultHandler(irqn: i16) {
	panic!("Unhandled exception (IRQn = {})", irqn);
}
