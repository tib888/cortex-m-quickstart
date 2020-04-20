use stm32f1xx_hal::can::{
    Can, Configuration, FilterBankConfiguration, FilterData, FilterInfo, FilterMode, Frame, Id,
    Payload, Pins, ReceiveFifo, RxFifo, RxFifo0, RxFifo1, TransmitMailbox, TxMailBox, TxMailBox0,
    TxMailBox1, TxMailBox2,
};

//debug only:
use core::fmt::Write;
use stm32f1xx_hal::pac::CAN1;

pub struct Messenger<CAN> {
    tx0: TxMailBox<CAN, TxMailBox0>,
    tx1: TxMailBox<CAN, TxMailBox1>,
    tx2: TxMailBox<CAN, TxMailBox2>,
    rx0: RxFifo<CAN, RxFifo0>,
    rx1: RxFifo<CAN, RxFifo1>,
}

//sensors
pub const ID_MOVEMENT: Id = Id::new_standard(0x700);
pub const ID_OPEN: Id = Id::new_standard(0x600);
pub const ID_IR: Id = Id::new_standard(0x500);
pub const ID_SWITCH: Id = Id::new_standard(0x400);
pub const ID_TEMPERATURE: Id = Id::new_standard(0x300);
pub const ID_LUX: Id = Id::new_standard(0x200);

//actors
pub const ID_RING: Id = Id::new_standard(0x100);
pub const ID_TARGET_TEMP: Id = Id::new_standard(0x080);
pub const ID_ROLL: Id = Id::new_standard(0x070);
pub const ID_LIGHT: Id = Id::new_standard(0x060);
pub const ID_SOUND: Id = Id::new_standard(0x050);
pub const ID_RGB: Id = Id::new_standard(0x040);
pub const ID_LED: Id = Id::new_standard(0x030);

//common
pub const ID_DATE: Id = Id::new_standard(0x020);
pub const ID_TIME: Id = Id::new_standard(0x010);

const CAN_CONFIG: Configuration = Configuration {
    time_triggered_communication_mode: false,
    automatic_bus_off_management: true,
    automatic_wake_up_mode: true,
    no_automatic_retransmission: false,
    receive_fifo_locked_mode: false, //??
    transmit_fifo_priority: false,   //??
    silent_mode: false,
    loopback_mode: false,
    synchronisation_jump_width: 1,
    bit_segment_1: 5,        //?3
    bit_segment_2: 5,        //?2
    time_quantum_length: 72, //?6
};

impl Messenger<CAN1> {
    pub fn new<PINS>(mut can: Can<CAN1, PINS>) -> Messenger<CAN1>
    where
        PINS: Pins<CAN1>,
    {
        can.configure(&CAN_CONFIG);
        nb::block!(can.to_normal()).unwrap(); //just to be sure

        let filter0 = FilterBankConfiguration {
            mode: FilterMode::List,
            info: FilterInfo::Halves((
                FilterData {
                    id: ID_MOVEMENT,
                    mask_or_id2: ID_OPEN,
                },
                FilterData {
                    id: ID_IR.clone(),
                    mask_or_id2: ID_SWITCH.clone(),
                },
            )),
            fifo_assignment: 0,
            active: true,
        };
        can.configure_filter_bank(0, &filter0);
        //let filter2 = FilterBankConfiguration {
        // 	mode: FilterMode::List,
        // 	info: FilterInfo::Whole(FilterData {
        // 		id: ID_TEMPERATURE.clone(),
        // 		mask_or_id2: ID_LUX.clone(),
        // 	}),
        // 	fifo_assignment: 0,
        // 	active: true,
        // };
        // can.configure_filter_bank(2, &filter2);

        let (can_tx, rx) = can.split();
        let (tx0, tx1, tx2) = can_tx.split();
        let (rx0, rx1) = rx.split();

        Messenger {
            tx0: tx0,
            tx1: tx1,
            tx2: tx2,
            rx0: rx0,
            rx1: rx1,
        }
    }

    pub fn transmit(&mut self, id: Id, payload: Payload) -> Result<(), ()> {
        if self.tx0.is_empty() {
            if let Ok(_) = self.tx0.request_transmit(&Frame::new(id, payload)) {
                return Ok(())
            }
            return Err(());
        }
        if self.tx1.is_empty() {
            if let Ok(_) = self.tx1.request_transmit(&Frame::new(id, payload)) {
                return Ok(())
            }
            return Err(());
        }
        if self.tx2.is_empty() {
            if let Ok(_) = self.tx2.request_transmit(&Frame::new(id, payload)) {
                return Ok(())
            }
            return Err(());
        }
        return Err(());
    }

    pub fn receive<T>(&mut self, tx: &mut T)
    where
        T: Write,
    {
        if let Ok((filter_match_index, time, frame)) = self.rx0.read() {
            writeln!(                
                tx,
                "rx0: f={} i={:x} t={} l={} d={}\r",
                filter_match_index,
                frame.id().standard(),
                time,
                frame.data().len(),
                frame.data().data_as_u64()
            )
            .unwrap();
        };

        if let Ok((filter_match_index, time, frame)) = self.rx1.read() {
            writeln!(
                tx,
                "rx1: f={} i={:x} t={} l={} d={}\r",
                filter_match_index,
                frame.id().standard(),
                time,
                frame.data().len(),
                frame.data().data_as_u64()
            )
            .unwrap();
        };
    }
}
