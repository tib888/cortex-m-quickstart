//! Models the roll operation

use core::ops::{Add, Sub};

#[derive(Copy, Clone, PartialEq)]
pub enum Command<DURATION> {
    SendUp,
    SendDown,
    SendTo(DURATION), //can be computed from bottom position if known
    Stop,
}

#[derive(Copy, Clone, PartialEq)]
pub enum State {
    DrivingUp,
    DrivingDown,
    Stopped,
}

pub struct Roll<DURATION> {
    state: State,
    current: DURATION,
    target: Option<DURATION>,
    bottom: Option<DURATION>,
}

impl<DURATION> Roll<DURATION> {
    pub fn new() -> Self
    where
        DURATION: Default,
    {
        Self {
            state: State::Stopped,
            current: DURATION::default(), //recalibrated when possible
            target: None,
            bottom: None, //recalibrated when possible
        }
    }

    pub fn update(&mut self, delta_t: DURATION, driving_current_detected: bool) -> State
    where
        DURATION: Default
            + PartialOrd
            + PartialEq
            + Add<DURATION, Output = DURATION>
            + Sub<DURATION, Output = DURATION>
            + Copy,
    {
        if delta_t != DURATION::default() {
            match self.state {
                State::Stopped => {}
                State::DrivingDown => {
                    self.current = self.current + delta_t;

                    if !driving_current_detected {
                        self.state = State::Stopped;
                        self.bottom = Some(self.current);
                        self.target = None;
                    } else {
                        if let Some(pos) = self.target {
                            if self.current >= pos {
                                self.target = None;
                                self.state = State::Stopped;
                            }
                        }
                    }
                }
                State::DrivingUp => {
                    if self.current > delta_t {
                        self.current = self.current - delta_t;
                    } else {
                        self.current = DURATION::default();
                    }

                    if !driving_current_detected {
                        self.state = State::Stopped;
                        self.current = DURATION::default();
                        self.target = None;
                    } else {
                        if let Some(pos) = self.target {
                            if self.current <= pos {
                                self.target = None;
                                self.state = State::Stopped;
                            }
                        }
                    }
                }
            }
        }
        self.state
    }

    pub fn execute(&mut self, command: Command<DURATION>)
    where
        DURATION: Default + PartialOrd + PartialEq + Copy,
    {
        match command {
            Command::Stop => {
                self.state = State::Stopped;
            }
            Command::SendUp => {
                self.target = None;
                self.state = State::DrivingUp;
            }
            Command::SendDown => {
                self.target = None;
                self.state = State::DrivingDown;
            }
            Command::SendTo(pos) => {
                if self.current < pos {
                    self.state = State::DrivingDown;
                } else if self.current > pos {
                    self.state = State::DrivingUp;
                } else {
                    self.state = State::Stopped;
                }
                self.target = Some(pos);
            }
        }
    }

    pub fn bottom<'a>(&'a self) -> &'a Option<DURATION> {
        &self.bottom
    }
}
