//! Switch d on A0 (pull down once in each main period if closed)
//! Switch c on A1 (pull down once in each main period if closed)
//! Switch a on A2 (pull down once in each main period if closed)
//! Switch b on A3 (pull down once in each main period if closed)
//!
//! Motion alarm on A4 (pull down)
//! Open alarm on A5 (pull down)
//!
//! A6, A7 not used, connected to the ground
//!
//! Optional piezzo speaker on A8
//!
//! Solid state relay connected to A9 drives the ssr_lamp_a
//! Solid state relay connected to A10 drives the ssr_lamp_b
//!
//! CAN (RX, TX) on A11, A12
//!
//! Read the NEC IR remote commands on A15 GPIO as input with internal pullup
//!
//! Photoresistor on B0 (ADC8)
//!
//! B1 not connected
//! B3 not used, connected to the ground
//!
//! DS18B20 1-wire temperature sensors connected to B4 GPIO
//! JTAG is removed from B3, B4 to make it work
//!
//! B5 not used, connected to the ground
//!
//! Solid state relay or arbitrary unit can be connected to B6, B7, B8, B9
//!
//! PT8211 DAC (BCK, DIN, WS) on B10, B11, B12
//!
//! RGB led on PB13, PB14, PB15 as push pull output
//!
//! C13 on board LED
//!
//! C14, C15 used on the bluepill board for 32768Hz xtal
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

use crate::hal::{
  can::*, delay::Delay, prelude::*, rtc, stm32f103xx, watchdog::IndependentWatchdog,
};
use crate::rt::{entry, ExceptionFrame};
use embedded_hal::{
  digital::InputPin,
  watchdog::{Watchdog, WatchdogEnable},
};
use ir::NecReceiver;
use onewire::*;
use room_pill::{
  ac_switch::*,
  ir_remote::*,
  rgb::{Colors, RgbLed},
  time::{Duration, Ticker, Ticks, Time},
};
//use sh::hio;
//use core::fmt::Write;

#[entry]
fn main() -> ! {
  door_unit_main();
}

fn door_unit_main() -> ! {
  let dp = stm32f103xx::Peripherals::take().unwrap();

  let mut watchdog = IndependentWatchdog::new(dp.IWDG);
  watchdog.start(2_000_000u32.us());

  let mut flash = dp.FLASH.constrain();

  //flash.acr.prftbe().enabled();//?? Configure Flash prefetch - Prefetch buffer is not available on value line devices
  //scb().set_priority_grouping(NVIC_PRIORITYGROUP_4);

  let mut rcc = dp.RCC.constrain();
  let clocks = rcc
    .cfgr
    .use_hse(8.mhz())
    .sysclk(72.mhz())
    .hclk(72.mhz())
    .pclk1(36.mhz())
    .pclk2(72.mhz())
    //.adcclk(12.mhz())
    .freeze(&mut flash.acr);

  watchdog.feed();

  // real time clock
  let rtc = rtc::Rtc::new(dp.RTC, &mut rcc.apb1, &mut dp.PWR);
  watchdog.feed();

  let mut afio = dp.AFIO.constrain(&mut rcc.apb2);
  // Disables the JTAG to free up pb3, pb4 and pa15 for normal use
  afio.mapr.disable_jtag();

  //configure pins:
  let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);

  //Switces on A0, A1, A2, A3 (pull down once in each main period if closed)
  let mut switch_d = AcSwitch::new(gpioa.pa0.into_pull_up_input(&mut gpioa.crl));
  let mut switch_c = AcSwitch::new(gpioa.pa1.into_pull_up_input(&mut gpioa.crl));
  let mut switch_a = AcSwitch::new(gpioa.pa2.into_pull_up_input(&mut gpioa.crl));
  let mut switch_b = AcSwitch::new(gpioa.pa3.into_pull_up_input(&mut gpioa.crl));

  // Motion alarm on A4 (pull down)
  let motion_alarm = gpioa.pa4.into_pull_up_input(&mut gpioa.crl);

  // Open alarm on A5 (pull down)
  let open_alarm = gpioa.pa5.into_pull_up_input(&mut gpioa.crl);

  // A6, A7 not used, connected to the ground
  let _a6 = gpioa.pa6.into_pull_down_input(&mut gpioa.crl);
  let _a7 = gpioa.pa7.into_pull_down_input(&mut gpioa.crl);

  // Optional piezzo speaker on A8
  let _piezzo = gpioa.pa8.into_open_drain_output(&mut gpioa.crh);

  // Solid state relay connected to A9 drives the lamp_b
  let mut ssr_lamp_b = gpioa.pa9.into_push_pull_output(&mut gpioa.crh);

  // Solid state relay connected to A10 drives the lamp_a
  let mut ssr_lamp_a = gpioa.pa10.into_push_pull_output(&mut gpioa.crh);

  // CAN (RX, TX) on A11, A12
  let canrx = gpioa.pa11.into_floating_input(&mut gpioa.crh);
  let cantx = gpioa.pa12.into_alternate_push_pull(&mut gpioa.crh);
  // USB is needed here because it can not be used at the same time as CAN since they share memory:
  let mut can = Can::can1(
    dp.CAN,
    (cantx, canrx),
    &mut afio.mapr,
    &mut rcc.apb1,
    dp.USB,
  );

  // Read the NEC IR remote commands on A15 GPIO as input with internal pullup
  let ir_receiver = gpioa.pa15.into_pull_up_input(&mut gpioa.crh);

  let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);

  // Photoresistor on B0 (ADC8)
  let photoresistor = gpiob.pb0.into_floating_input(&mut gpiob.crl);

  // B1 not connected
  let _b1 = gpiob.pb1.into_pull_down_input(&mut gpiob.crl);

  // B3 not used, connected to the ground
  let _b3 = gpiob.pb3.into_pull_down_input(&mut gpiob.crl);

  // DS18B20 1-wire temperature sensors connected to B4 GPIO
  let onewire_io = gpiob.pb4.into_open_drain_output(&mut gpiob.crl);

  // B5 not used, connected to the ground
  let _b5 = gpiob.pb5.into_pull_down_input(&mut gpiob.crl);

  // Solid state relay or arbitrary unit can be connected to B6, B7, B8, B9
  let _ssr_0 = gpiob.pb6.into_push_pull_output(&mut gpiob.crl);
  let _ssr_1 = gpiob.pb7.into_push_pull_output(&mut gpiob.crl);
  let _ssr_2 = gpiob.pb8.into_push_pull_output(&mut gpiob.crh);
  let _ssr_3 = gpiob.pb9.into_push_pull_output(&mut gpiob.crh);

  // PT8211 DAC (BCK, DIN, WS) on B10, B11, B12
  let dac = room_pill::dac::Pt8211::new(
    gpiob.pb10.into_push_pull_output(&mut gpiob.crh), //use as SCL?
    gpiob.pb11.into_push_pull_output(&mut gpiob.crh), //use as SDA?
    gpiob.pb12.into_push_pull_output(&mut gpiob.crh), //word select (left / right^)
  );

  // RGB led on PB13, PB14, PB15 as push pull output
  let mut rgb = RgbLed::new(
    gpiob.pb13.into_push_pull_output(&mut gpiob.crh),
    gpiob.pb14.into_push_pull_output(&mut gpiob.crh),
    gpiob.pb15.into_push_pull_output(&mut gpiob.crh),
  );
  rgb.color(Colors::Black);

  let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);

  // C13 on board LED^
  let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

  // C14, C15 used on the bluepill board for 32768Hz xtal

  watchdog.feed();

  let cp = cortex_m::Peripherals::take().unwrap();

  let mut flash = dp.FLASH.constrain();
  let clocks = rcc.cfgr.freeze(&mut flash.acr);
  let mut delay = Delay::new(cp.SYST, clocks);
  let mut one_wire = OneWirePort::new(onewire_io, delay);

  let tick = Ticker::new(cp.DWT, cp.DCB, clocks);
  let mut receiver = ir::IrReceiver::<Time<Ticks>>::new();

  let mut last_time = tick.now();

  let ac_period = Duration::<room_pill::time::Ticks>::from(tick.frequency / 50);

  //main update loop
  loop {
    watchdog.feed();

    //update the IR receiver statemachine:
    let ir_cmd = receiver.receive(&tick, tick.now(), ir_receiver.is_low());

    match ir_cmd {
      Ok(ir::NecContent::Repeat) => {}
      Ok(ir::NecContent::Data(data)) => {
        let command = translate(data);
        //write!(hstdout, "{:x}={:?} ", data, command).unwrap();
        //model.ir_remote_command(command, &MENU);
        //model.refresh_display(&mut display, &mut backlight);
      }
      _ => {}
    }

    // calculate the time since last execution:
    let delta = tick.now() - last_time;

    switch_a.update(ac_period, delta);
    switch_b.update(ac_period, delta);
    switch_c.update(ac_period, delta);
    switch_d.update(ac_period, delta);

    if let (Some(last), Some(current)) = (switch_a.last_state(), switch_a.state()) {
      if last != current {
        ssr_lamp_a.toggle();
      }
    };

    if let (Some(last), Some(current)) = (switch_b.last_state(), switch_b.state()) {
      if last != current {
        ssr_lamp_b.toggle();
      }
    };

    // do not execute the followings too often: (temperature conversion time of the sensors is a lower limit)
    if delta.count < tick.frequency {
      continue;
    }

    led.toggle();
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
