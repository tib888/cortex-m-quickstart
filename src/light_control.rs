use embedded_hal::digital::v2::{InputPin, OutputPin, StatefulOutputPin};

#[derive(Clone, Copy)]
pub enum Mode {
    ///remote can change the state only
    Constant,
    ManualOnly,
    MovementOnly,
    ///possible to switch between the modes by a quick on-off or off-on
    Mixed(bool), //true = movement, false = manual
}

struct State {
    mode: Mode,
    current_lamp: bool,
    last_switch: bool,
    transition_time: u32,   //ms
    last_turn_on_time: u32, //sec
    switch_lamp_inconsistency: bool,
}

impl State {
    fn new(mode: Mode, current_lamp: bool, current_switch: bool, current_time: u32) -> Self {
        Self {
            mode: mode,
            current_lamp: current_lamp,
            last_switch: current_switch,
            transition_time: current_time,   //ms
            last_turn_on_time: current_time, //sec
            switch_lamp_inconsistency: (current_lamp != current_switch),
        }
    }

    fn set(&mut self, on: bool, t: u32, current_switch: bool) -> bool {
        if on {
            if !self.current_lamp {
                self.last_turn_on_time = t;
                self.current_lamp = true;
            }
        } else {
            self.current_lamp = false
        }

        self.last_switch = current_switch;
        self.switch_lamp_inconsistency = self.current_lamp != current_switch;
        self.current_lamp
    }

    //normal periodically called update
    fn update(
        &mut self,
        mode_switch_timeout: u32,
        movement_timeout: u32,
        manual_timeout: Option<u32>,
        t: u32,
        current_switch: bool,
        current_movement: bool,
        current_lamp: bool,
    ) -> bool {
        if let Mode::Mixed(auto) = self.mode {
            let transition = self.last_switch != current_switch;
            if t <= self.transition_time + mode_switch_timeout {
                if transition {
                    self.transition_time = t;
                    //a fast back and forth transition flips auto <=> manual modes
                    self.mode = Mode::Mixed(!auto);
                }
                //wait mode decision stabilization
                return current_lamp;
            }
            if transition {
                self.transition_time = t;
            }
        }

        let on = match self.mode {
            Mode::ManualOnly | Mode::Mixed(false) => {
                if let Some(timeout) = manual_timeout {
                    (t <= self.last_turn_on_time + timeout)
                        && (current_switch ^ self.switch_lamp_inconsistency)
                } else {
                    (current_switch ^ self.switch_lamp_inconsistency)
                }
            }

            Mode::MovementOnly | Mode::Mixed(true) => {
                if current_movement {
                    self.last_turn_on_time = t;
                }
                t <= self.last_turn_on_time + movement_timeout
            }

            Mode::Constant => self.current_lamp,
        };

        self.set(on, t, current_switch)
    }
}

pub struct Controller<SWPIN, RELAYPIN, MOVEPIN, ERROR>
where
    SWPIN: InputPin<Error = ERROR>,
    MOVEPIN: InputPin<Error = ERROR>,
    RELAYPIN: OutputPin<Error = ERROR>,
{
    switch: SWPIN,
    lamp: RELAYPIN,
    movement: MOVEPIN,

    movement_timeout: u32,       //Duration in sec
    manual_timeout: Option<u32>, //Duration in sec

    state: State,
}

impl<SWPIN, RELAYPIN, MOVEPIN, ERROR> Controller<SWPIN, RELAYPIN, MOVEPIN, ERROR>
where
    SWPIN: InputPin<Error = ERROR>,
    MOVEPIN: InputPin<Error = ERROR>,
    RELAYPIN: OutputPin<Error = ERROR> + StatefulOutputPin,
{
    pub fn new(switch: SWPIN, lamp: RELAYPIN, movement: MOVEPIN, mode: Mode) -> Result<Self, ERROR> {
        let lighting = lamp.is_set_high()?;
        let switched = switch.is_high()?;
        let t = 0;
        Ok(Controller {
            switch,
            lamp,
            movement,
            movement_timeout: 2 * 60,
            manual_timeout: Some(30 * 60),
            state: State::new(mode, lighting, switched, t),
        })
    }

    pub fn update(&mut self, t: u32) -> Result<(), ERROR> {
        let on = self.state.update(
            0u32,
            self.movement_timeout,
            self.manual_timeout,
            t,
            self.switch.is_high()?,
            self.movement.is_high()?,
            self.lamp.is_set_high()?,
        );
        if on {
            self.lamp.set_high()
        } else {
            self.lamp.set_low()
        }
    }

    pub fn current_mode(&self) -> Mode {
        self.state.mode
    }

    pub fn is_lighting(&self) -> Result<bool, ERROR> {
        self.lamp.is_set_high()
    }

    pub fn set_lighting(&mut self, on: bool, t: u32) -> Result<(), ERROR> {
        let on = self.state.set(on, t, self.switch.is_high()?);

        if on {
            self.lamp.set_high()
        } else {
            self.lamp.set_low()
        }
    }
}
