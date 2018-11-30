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
extern crate ir;
extern crate lcd_hal;
extern crate nb;
extern crate onewire;
extern crate panic_semihosting;
extern crate room_pill;
extern crate stm32f103xx as device;
extern crate stm32f103xx_hal as hal;

use core::fmt::Write;
use core::marker::PhantomData;
use embedded_hal::watchdog::{Watchdog, WatchdogEnable};
use hal::can::*;
use hal::delay::Delay;
use hal::prelude::*;
use hal::rtc;
use hal::stm32f103xx;
use hal::watchdog::IndependentWatchdog;
use ir::NecReceiver;
use lcd_hal::{hx1230, hx1230::Hx1230, Display};
use onewire::ds18x20::*;
use onewire::temperature::Temperature;
use onewire::*;
use room_pill::floor_heating;
use room_pill::pump::*;
use room_pill::rgb::*;
use room_pill::time::{Duration, Seconds, Ticker, Ticks, Time};
use room_pill::valve::*;
use room_pill::week_time::*;
use rt::ExceptionFrame;
use sh::hio; //

#[derive(Debug, PartialEq)]
enum IR_Commands {
    N_0,
    N_1,
    N_2,
    N_3,
    N_4,
    N_5,
    N_6,
    N_7,
    N_8,
    N_9,

    Ok,

    Up,
    Down,
    Left,
    Right,

    Home,
    Back,
    Menu,
    Click,

    Mute,
    Backspace,
    Power,

    Red,
    Green,
    Yellow,
    Blue,

    Power_,
    Set_,
    TVIn_,
    VolDown_,
    VolUp_,

    Unknown,
}

struct SubTree<'a, M> {
    items: &'a [TreeNode<'a, M>],
}

struct Leaf<M> {
    update: fn(model: &mut M, command: IR_Commands),
    view: fn(model: &M) -> &[u8],
}

enum TreeContent<'a, M> {
    SubTree(SubTree<'a, M>),
    Leaf(Leaf<M>),
}

struct TreeNode<'a, M> {
    text: &'a [u8],
    content: TreeContent<'a, M>,
}

fn dummy_update(_model: &mut Model, _command: IR_Commands) {
    //TODO delete this later
}
fn dummy_view(_model: &Model) -> &'static [u8] {
    b"*dummy*" //TODO delete this later
}

fn set_time_weekday(model: &mut Model, command: IR_Commands) {
    match command {
        IR_Commands::Right => {
            model.weektime = WeekTime {
                weekday: (model.weektime.weekday + 1) % 7,
                ..model.weektime
            };
            model.update_time_offset();
        }
        IR_Commands::Down => {
            model.weektime = WeekTime {
                weekday: (model.weektime.weekday - 1) % 7,
                ..model.weektime
            };
            model.update_time_offset();
        }
        _ => {}
    };
}

fn fmt_nn(n: u8) -> &'static [u8] {
    assert!(n < 100);
    static mut TEXT: [u8; 2] = [0u8; 2];
    unsafe {
        TEXT[0] = '0' as u8 + (n / 10u8);
        TEXT[1] = '0' as u8 + (n % 10u8);
        &TEXT
    }
}

fn weekday_view(model: &Model) -> &'static [u8] {
    WEEKDAYS[model.weektime.weekday as usize]
}
fn hour_view(model: &Model) -> &'static [u8] {
    fmt_nn(model.weektime.hour)
}
fn minute_view(model: &Model) -> &'static [u8] {
    fmt_nn(model.weektime.min)
}

fn set_time_hour(model: &mut Model, command: IR_Commands) {
    match command {
        IR_Commands::Right => {
            model.weektime = WeekTime {
                hour: (model.weektime.hour + 1) % 24,
                ..model.weektime
            }
        }
        IR_Commands::Down => {
            model.weektime = WeekTime {
                hour: (model.weektime.hour - 1) % 24,
                ..model.weektime
            }
        }
        _ => {
            return;
        }
    }
    model.update_time_offset();
}

fn set_time_min(model: &mut Model, command: IR_Commands) {
    match command {
        IR_Commands::Right => {
            model.weektime = WeekTime {
                min: (model.weektime.min + 1) % 60,
                sec: 0,
                ..model.weektime
            }
        }
        IR_Commands::Down => {
            model.weektime = WeekTime {
                min: (model.weektime.min - 1) % 60,
                sec: 0,
                ..model.weektime
            }
        }
        _ => {
            return;
        }
    }
    model.update_time_offset();
}

const MAX_COUNT: usize = 4;

struct Model<'a, 'b> {
    //config:
    can_config: Configuration,
    floor_heating_config: floor_heating::Config<Temperature, Duration<Seconds>>,
    backlight_timeout: Duration<Seconds>, //time in seconds before backlight tuns off
    time_offset: u32,                     //used for rtc to weektime calibration

    //state:
    floor_heating_state: floor_heating::State<Duration<Seconds>>,
    temperatures: [Option<Temperature>; MAX_COUNT],
    time: Time<Seconds>, //rtc based, ever increasing, in seconds
    weektime: WeekTime,  //redundant WeekTime::from(self.time + self.time_offset)

    //UI state:
    active_menu: Option<&'b SubTree<'a, Model<'a, 'b>>>,
    selected_item: usize,
}

impl<'a, 'b> Model<'a, 'b> {
    fn new() -> Self {
        Self {
            can_config: Configuration {
                time_triggered_communication_mode: false,
                automatic_bus_off_management: true,
                automatic_wake_up_mode: true,
                no_automatic_retransmission: false,
                receive_fifo_locked_mode: false,
                transmit_fifo_priority: false,
                silent_mode: false,
                loopback_mode: false,
                synchronisation_jump_width: 1,
                bit_segment_1: 3,
                bit_segment_2: 2,
                time_quantum_length: 6,
            },

            //this config will be updated by IR remote or CAN messages
            floor_heating_config: floor_heating::Config {
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
            },

            backlight_timeout: Duration::<Seconds>::hms(0, 1, 0),

            time_offset: 0u32,

            floor_heating_state: floor_heating::State::Standby(Duration::sec(0)),
            temperatures: [None; MAX_COUNT],
            time: Time::<Seconds> {
                instant: 0,
                unit: PhantomData::<Seconds>,
            },
            weektime: WeekTime::default(),

            active_menu: None,
            selected_item: 0,
        }
    }

    ///set weektime from self.time.instant + self.time_offset
    fn update_weektime(&mut self) {
        self.weektime = WeekTime::from(Time::<Seconds>::from(
            self.time.instant.wrapping_add(self.time_offset),
        ));
    }

    ///set timeoffset from self.weektime - self.time.instant
    fn update_time_offset(&mut self) {
        self.time_offset = Time::<Seconds>::from(self.weektime)
            .instant
            .wrapping_sub(self.time.instant);
    }

    //update by real time clock
    fn update_time(&mut self, time: Time<Seconds>) {
        if self.time != time {
            let delta_time = time - self.time;
            self.time = time;

            self.floor_heating_state = self.floor_heating_state.update(
                &self.floor_heating_config,
                self.temperatures[0],
                self.temperatures[1],
                self.temperatures[2],
                self.temperatures[3],
                delta_time,
            );

            if self.backlight_timeout > Duration::<Seconds>::default() {
                if self.backlight_timeout < delta_time {
                    self.backlight_timeout = Duration::<Seconds>::default();
                } else {
                    self.backlight_timeout = self.backlight_timeout - delta_time;
                }
            }

            self.update_weektime();
        }
    }

    //update by IR remote
    fn ir_remote_command(
        &mut self,
        command: IR_Commands,
        root_menu: &'a SubTree<'a, Model<'a, '_>>,
    ) {
        self.backlight_timeout = Duration::hms(0, 0, 20);

        if let Some(active_menu) = self.active_menu {
            let n = active_menu.items.len();
            match command {
                IR_Commands::Home => self.active_menu = None,
                IR_Commands::Up => if self.selected_item > 0 {
                    self.selected_item -= 1;
                } else if n > 0 {
                    self.selected_item = n - 1;
                },
                IR_Commands::Down => {
                    if self.selected_item + 1 < n {
                        self.selected_item += 1;
                    } else {
                        self.selected_item = 0;
                    }
                }
                IR_Commands::Ok => {
                    if let TreeContent::SubTree(ref subtree) =
                        active_menu.items[self.selected_item].content
                    {
                        self.active_menu = Some(&subtree);
                        self.selected_item = 0;
                    }
                }
                _ => {
                    if let TreeContent::Leaf(ref leaf) =
                        active_menu.items[self.selected_item].content
                    {
                        (leaf.update)(self, command);
                    }
                }
            }
        } else if IR_Commands::Menu == command {
            self.active_menu = Some(root_menu);
            self.selected_item = 0;
        }
    }

    //update by temp sensors
    fn update_temperature(&mut self, index: usize, temperature: Option<Temperature>) {
        self.temperatures[index] = temperature;
    }

    fn refresh_display<D: lcd_hal::Display, B: embedded_hal::digital::OutputPin>(
        &self,
        display: &mut D,
        backlight: &mut B,
    ) {
        if self.backlight_timeout == Duration::default() {
            backlight.set_high(); //turn off
        } else {
            backlight.set_low(); //turn on
        }

        if let Some(active_menu) = self.active_menu {
            //TODO render menu
            display.clear();
            // TODO let (cols, rows) = display.get_char_resolution();
            let rows = 8;
            let start_index = if self.selected_item >= rows {
                self.selected_item - rows + 1
            } else {
                0
            };

            for row in 0..rows {
                let index = row + start_index;
                if index >= active_menu.items.len() {
                    break;
                }

                display.set_position(0, row as u8);

                display.print_char(if self.selected_item == index {
                    '>' as u8
                } else {
                    ' ' as u8
                });
                display.print(active_menu.items[index].text);

                if let TreeContent::Leaf(ref leaf) = active_menu.items[index].content {
                    display.print_char(':' as u8);
                    display.print((leaf.view)(self));
                }
            }
        } else {
            //display status
            display.clear();

            display.set_position(0, 0);
            print_time(display, self.weektime);

            print_temp(
                display,
                3,
                b"Cel:    ",
                &self.floor_heating_config.target_air_temperature,
            );

            static LABELS: [&[u8]; MAX_COUNT] =
                [b"Elore:  ", b"Vissza: ", b"Padlo:  ", b"Levego: "];

            for i in 0..4 as u8 {
                print_temp(
                    display,
                    4 + i,
                    LABELS[i as usize],
                    &self.temperatures[i as usize],
                );
            }

            //display.set_position(90, 7);
            //display.print(b" TTTTTTTTTTTTTTTT");
        }
    }
}

entry!(main);

fn print_temp<D: lcd_hal::Display>(
    display: &mut D,
    row: u8,
    prefix: &[u8],
    temp: &Option<Temperature>,
) {
    display.set_position(0, row);
    display.print(prefix);

    if let Some(temp) = temp {
        let t = temp.whole_degrees();
        display.print_char(if t < 0 { '-' } else { ' ' } as u8);

        let t: u8 = t.abs() as u8;
        print_nn(display, t);
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
        static ROUND_TABLE1: &[u8] = b"0011233455667889";
        static ROUND_TABLE2: &[u8] = b"0639518406395184";
        display.print_char(ROUND_TABLE1[temp.fraction_degrees() as usize]);
        display.print_char(ROUND_TABLE2[temp.fraction_degrees() as usize]);
    } else {
        display.print(b" -----");
    }
}

fn print_nn<D: lcd_hal::Display>(display: &mut D, n: u8) {
    assert!(n < 100);
    display.print_char('0' as u8 + (n / 10u8));
    display.print_char('0' as u8 + (n % 10u8));
}

pub static WEEKDAYS: [&[u8]; 7] = [
    b"Hetfo    ",
    b"Kedd     ",
    b"Szerda   ",
    b"Csutortok",
    b"Pentek   ",
    b"Szombat  ",
    b"Vasarnap ",
];

fn print_time<D: lcd_hal::Display>(display: &mut D, t: WeekTime) {
    print_nn(display, t.hour);
    display.print_char(':' as u8);
    print_nn(display, t.min);
    display.print_char(' ' as u8);
    display.print(WEEKDAYS[t.weekday as usize]);
}

fn main() -> ! {
    let menu = SubTree {
        items: &[
            TreeNode {
                text: b"Program",
                content: TreeContent::Leaf(Leaf {
                    update: dummy_update,
                    view: dummy_view,
                }),
            },
            TreeNode {
                text: b"Set Clock",
                content: TreeContent::SubTree(SubTree {
                    items: &[
                        TreeNode {
                            text: b"Weekday",
                            content: TreeContent::Leaf(Leaf {
                                update: set_time_weekday,
                                view: weekday_view,
                            }),
                        },
                        TreeNode {
                            text: b"Hour",
                            content: TreeContent::Leaf(Leaf {
                                update: set_time_hour,
                                view: hour_view,
                            }),
                        },
                        TreeNode {
                            text: b"Min",
                            content: TreeContent::Leaf(Leaf {
                                update: set_time_min,
                                view: minute_view,
                            }),
                        },
                    ],
                }),
            },
            TreeNode {
                text: b"Config",
                content: TreeContent::Leaf(Leaf {
                    update: dummy_update,
                    view: dummy_view,
                }),
            },
            TreeNode {
                //reset command: rescan temp sensors, reset display at least, but keeps program, config
                text: b"Reset",
                content: TreeContent::Leaf(Leaf {
                    update: dummy_update,
                    view: dummy_view,
                }),
            },
        ],
    };

    let mut model = Model::new();

    let mut dp = stm32f103xx::Peripherals::take().unwrap();

    let mut watchdog = IndependentWatchdog::new(dp.IWDG);
    watchdog.start(2_000_000u32.us());

    //let mut hstdout = hio::hstdout().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let mut rcc = dp.RCC.constrain();

    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
    let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);

    // real time clock
    let rtc = rtc::Rtc::new(dp.RTC, &mut rcc.apb1, &mut dp.PWR);

    // IR receiver^
    let ir_receiver = gpioa.pa15.into_pull_up_input(&mut gpioa.crh);

    // RGB led:
    let mut rgb = RgbLed::new(
        gpiob.pb13.into_open_drain_output(&mut gpiob.crh),
        gpiob.pb14.into_open_drain_output(&mut gpiob.crh),
        gpiob.pb15.into_open_drain_output(&mut gpiob.crh),
    );

    let mut heat_request = gpiob.pb11.into_push_pull_output(&mut gpiob.crh);

    // on board led^:
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    // valve opener SSR^
    let mut valve = ValveSSR::new(gpiob.pb6.into_open_drain_output(&mut gpiob.crl));

    // pump starter SSR^
    let mut pump = PumpSSR::new(gpiob.pb7.into_open_drain_output(&mut gpiob.crl));

    let mut flash = dp.FLASH.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);

    watchdog.feed();

    // setup SPI for the lcd display:
    let sck = gpioa.pa5.into_push_pull_output(&mut gpioa.crl); //PA5 = Display SPI clock
    let mosi = gpioa.pa7.into_push_pull_output(&mut gpioa.crl); //PA7 = Display SPI data

    // other pins for lcd
    let mut backlight = gpiob.pb12.into_open_drain_output(&mut gpiob.crh); //PB12 Display backlight^
    backlight.set_low();

    let cs = gpioa.pa2.into_push_pull_output(&mut gpioa.crl); // PA3 = Display ChipSelect^
    let mut rst = gpioa.pa1.into_push_pull_output(&mut gpioa.crl); // PA1 = Display Reset^

    let mut delay = Delay::new(cp.SYST, clocks);
    let mut display = hx1230::gpio::Hx1230Gpio::new(sck, mosi, cs, &mut rst, &mut delay);
    display.init();
    display.set_contrast(7);

    //rotate the screen with 180 degree:
    //display.flip_horizontal(true);
    //display.flip_vertical(true);

    watchdog.feed();

    // setup the one wire thermometers:
    // free PB3, PB4 from JTAG to be used as GPIO:
    afio.mapr
        .mapr()
        .modify(|_, w| unsafe { w.swj_cfg().bits(1) });
    let io = gpiob.pb4.into_open_drain_output(&mut gpiob.crl);
    let mut one_wire = OneWirePort::new(io, delay);

    watchdog.feed();

    let tick = Ticker::new(cp.DWT, cp.DCB, clocks);

    let mut receiver = ir::IrReceiver::<Time<Ticks>>::new();

    let canrx = gpioa.pa11.into_floating_input(&mut gpioa.crh);
    let cantx = gpioa.pa12.into_alternate_push_pull(&mut gpioa.crh);

    //remapped version:
    //let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
    //let canrx = gpiob.pb8.into_floating_input(&mut gpiob.crh);
    //let cantx = gpiob.pb9.into_alternate_push_pull(&mut gpiob.crh);

    //USB is needed here because it can not be used at the same time as CAN since they share memory:
    let mut can = Can::can1(
        dp.CAN,
        (cantx, canrx),
        &mut afio.mapr,
        &mut rcc.apb1,
        dp.USB,
    );

    watchdog.feed();
    can.configure(&model.can_config);

    watchdog.feed();
    nb::block!(can.to_normal()).unwrap(); //just to be sure

    watchdog.feed();
    let can_reconfigure_id: Id = Id::new_standard(13);
    let can_ask_status_id: Id = Id::new_standard(14);
    let _can_heat_request_id: Id = Id::new_standard(15);
    let _can_temperature_report_id: Id = Id::new_standard(16);

    let filterbank0_config = FilterBankConfiguration {
        mode: FilterMode::List,
        info: FilterInfo::Whole(FilterData {
            id: can_reconfigure_id.clone(),
            mask_or_id2: can_ask_status_id.clone(), //with_rtr()
        }),
        fifo_assignment: 0, //depending on this rx0 or rx1 will be targeted
        active: true,
    };
    can.configure_filter_bank(0, &filterbank0_config);

    let (tx, rx) = can.split();

    let (mut _tx0, mut _tx1, mut _tx2) = tx.split();
    let (mut rx0, mut _rx1) = rx.split();

    watchdog.feed();

    //store the addresses of temp sensors, start measurement on each:
    let mut roms = [[0u8; 8]; MAX_COUNT];
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
                    if count >= MAX_COUNT {
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

    let mut last_time = tick.now();

    let mut hstdout = hio::hstdout().unwrap();

    loop {
        watchdog.feed();

        //receive and process can messages
        if let Ok((filter_match_index, _time, frame)) = rx0.read() {
            // writeln!(
            //     hstdout,
            //     "rx0: {} {} {} {} {}",
            //     filter_match_index,
            //     frame.id().standard(),
            //     time,
            //     frame.data().len(),
            //     frame.data().data_as_u64()
            // ).unwrap();

            match filter_match_index {
                0 => assert!(*frame.id() == can_reconfigure_id), //TODO decode new config
                1 => assert!(*frame.id() == can_ask_status_id),  //TODO send status on can
                _ => {} //panic!("unexpected"),
            }
        };

        // if let Ok((filter_match_index, time, frame)) = rx1.read() {
        //     ...
        // };

        //update the IR receiver statemachine:
        let ir_cmd = receiver.receive(tick.now(), ir_receiver.is_low());

        match ir_cmd {
            Ok(ir::NecContent::Repeat) => {}
            Ok(ir::NecContent::Data(data)) => {
                let command = match data >> 8 {
                    0x807F02 => IR_Commands::Power,
                    0x807FAA => IR_Commands::Power_,

                    0x807F9A => IR_Commands::Set_,
                    0x807F1A => IR_Commands::TVIn_,
                    0x807FEA => IR_Commands::VolDown_,
                    0x807F6A => IR_Commands::VolUp_,

                    0x807Fc2 => IR_Commands::Red,
                    0x807Ff0 => IR_Commands::Green,
                    0x807F08 => IR_Commands::Yellow,
                    0x807F18 => IR_Commands::Blue,

                    0x807F88 => IR_Commands::Home,
                    0x807F98 => IR_Commands::Back,
                    0x807F32 => IR_Commands::Menu,
                    0x807F00 => IR_Commands::Click,

                    0x807Fc8 => IR_Commands::Ok,

                    0x807F68 => IR_Commands::Up,
                    0x807F58 => IR_Commands::Down,
                    0x807F8A => IR_Commands::Left,
                    0x807F0A => IR_Commands::Right,

                    0x807F72 => IR_Commands::N_1,
                    0x807Fb0 => IR_Commands::N_2,
                    0x807F30 => IR_Commands::N_3,

                    0x807F52 => IR_Commands::N_4,
                    0x807F90 => IR_Commands::N_5,
                    0x807F10 => IR_Commands::N_6,

                    0x807F62 => IR_Commands::N_7,
                    0x807Fa0 => IR_Commands::N_8,
                    0x807F20 => IR_Commands::N_9,

                    0x807F82 => IR_Commands::Mute,
                    0x807F80 => IR_Commands::N_0,
                    0x807F42 => IR_Commands::Backspace,

                    _ => IR_Commands::Unknown,
                };
                //write!(hstdout, "{:x}={:?} ", data, command).unwrap();
                model.ir_remote_command(command, &menu);
                model.refresh_display(&mut display, &mut backlight);
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

        // keep the difference measurement accurate by keeping the fractions...
        last_time = last_time + Duration::<Ticks>::from(delta_time.count * tick.frequency);

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

        // and an independent real time clock with 1 sec resolution:
        model.update_time(Time::<Seconds> {
            instant: rtc.get_cnt(),
            unit: PhantomData::<Seconds>,
        });

        // drive outputs, send messages:

        // let txresult0 = tx0.request_transmit(&Frame::new(
        //     can_temperature_report_id,
        //     Payload::new(temp_sensors[0]),
        // ));
        // let txresult1 = tx1.request_transmit(&Frame::new(
        //     can_heat_request_id, Payload::new(true)
        // ));
        // let txresult2 = tx2.request_transmit(&Frame::new(
        //     can_reconfigure_id,
        //     Payload::new(floor_heating_config.target_air_temperature),
        // ));

        let _status_text = match model.floor_heating_state {
            floor_heating::State::Heating(defreeze) => {
                valve.open();
                pump.start();
                heat_request.set_high();
                //CAN: heat request
                if defreeze {
                    rgb.color(Colors::Purple);
                    "Olvasztas"
                } else {
                    rgb.color(Colors::Red);
                    "Futes"
                }
            }
            floor_heating::State::AfterCirculation(_) => {
                valve.close();
                pump.start();
                heat_request.set_low();
                //CAN: no heat request
                rgb.color(Colors::Yellow);
                "Utokeringetes"
            }
            floor_heating::State::Standby(_) => {
                valve.close();
                pump.stop();
                heat_request.set_low();
                //CAN: no heat request
                rgb.color(Colors::Green);
                "Keszenlet"
            }
            floor_heating::State::FreezeProtectionCheckCirculation(_) => {
                valve.close();
                pump.start();
                heat_request.set_low();
                //CAN: no heat request
                rgb.color(Colors::Blue);
                "Fagyvizsgalat"
            }
            floor_heating::State::Error => {
                //CAN: sensor missing error
                rgb.color(Colors::Cyan);
                "Szenzorhiba"
            }
        };

        model.refresh_display(&mut display, &mut backlight);

        if backlight.is_high() {
            //exit from menu when backlight timed out
            model.active_menu = None;
        }

        //display.set_position(0, 2);
        //display.print(_status_text);
    }
}

exception!(HardFault, hard_fault);

fn hard_fault(_ef: &ExceptionFrame) -> ! {
    loop {}
    //panic!("HardFault at {:#?}", ef); //removed due to large code size
}

exception!(*, default_handler);

fn default_handler(_irqn: i16) {
    //panic!("Unhandled exception (IRQn = {})", irqn);  //removed due to large code size
}
