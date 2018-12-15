use core::ops::Add;
use core::ops::Sub;

pub enum State<Duration> {
    PrepareHeating((bool, Duration)), //bool=defrost, duration required to fully open the valve before starting the heater
    Heating(bool),                    //bool=defrost
    AfterCirculation(Duration),       //circulation_since_closed duration
    Standby(Duration),                //since_last_freeze_test duration
    FreezeProtectionCheckCirculation(Duration), //pre_circulation_duration
    Error,
}

pub struct FreezeProtectionConfig<Temperature, Duration> {
    pub min_temperature: Temperature,
    pub safe_temperature: Temperature,
    pub check_interval: Duration,
    pub check_duration: Duration,
}

pub struct Config<Temperature, Duration> {
    pub max_forward_temperature: Temperature,
    pub max_floor_temperature: Temperature,
    pub target_air_temperature: Option<Temperature>,
    pub temperature_histeresis: Temperature,
    pub freeze_protection: FreezeProtectionConfig<Temperature, Duration>,
    pub pre_circulation_duration: Duration,
    pub after_circulation_duration: Duration,
}

impl<Duration: Copy + PartialOrd + Default + Add<Duration, Output = Duration>> State<Duration> {
    pub fn update<
        Temperature: Copy
            + PartialOrd
            + Add<Temperature, Output = Temperature>
            + Sub<Temperature, Output = Temperature>,
    >(
        &self,
        config: &Config<Temperature, Duration>,
        forward_temperature: Option<Temperature>,
        return_temperature: Option<Temperature>,
        floor_temperature: Option<Temperature>,
        air_temperature: Option<Temperature>,
        delta_time: Duration,
    ) -> State<Duration> {
        match self {
            State::PrepareHeating((defreeze, circulation_since_opened)) => {
                if *circulation_since_opened > config.pre_circulation_duration {
                    State::Heating(*defreeze)
                } else {
                    State::PrepareHeating((*defreeze, *circulation_since_opened + delta_time))
                }
            }

            State::Heating(defreeze) => {
                //too hot protection:
                if let Some(ref forward_temp) = forward_temperature {
                    if *forward_temp >= config.max_forward_temperature {
                        return State::AfterCirculation(Duration::default());
                    }
                }
                if let Some(ref floor_temp) = floor_temperature {
                    if *floor_temp >= config.max_floor_temperature {
                        return State::AfterCirculation(Duration::default());
                    }
                }

                if *defreeze {
                    let return_temp = if let Some(ref temp) = return_temperature {
                        temp
                    } else if let Some(ref temp) = air_temperature {
                        //use as a backup sensor
                        temp
                    } else {
                        return State::Error;
                    };

                    if *return_temp >= config.freeze_protection.safe_temperature {
                        State::AfterCirculation(Duration::default())
                    } else {
                        State::Heating(true)
                    }
                } else {
                    if let Some(ref target) = config.target_air_temperature {
                        let current_temp = if let Some(ref temp) = air_temperature {
                            temp
                        } else if let Some(ref temp) = return_temperature {
                            temp //use a backup sensor
                        } else {
                            return State::Error;
                        };

                        if *current_temp > (*target + config.temperature_histeresis) {
                            State::AfterCirculation(Duration::default())
                        } else {
                            State::Heating(false)
                        }
                    } else {
                        State::Standby(Duration::default())
                    }
                }
            }

            State::AfterCirculation(circulation_since_closed) => {
                if *circulation_since_closed > config.after_circulation_duration {
                    State::Standby(Duration::default())
                } else {
                    State::AfterCirculation(*circulation_since_closed + delta_time)
                }
            }

            State::Standby(since_last_freeze_test) => {
                if let Some(target) = config.target_air_temperature {
                    if let Some(temp) = air_temperature {
                        if temp <= target - config.temperature_histeresis {
                            return State::PrepareHeating((false, Duration::default()));
                        }
                    };
                };

                if *since_last_freeze_test > config.freeze_protection.check_interval {
                    State::FreezeProtectionCheckCirculation(Duration::default())
                } else {
                    State::Standby(*since_last_freeze_test + delta_time)
                }
            }

            State::FreezeProtectionCheckCirculation(circulation_duration) => {
                let return_temp = if let Some(temp) = return_temperature {
                    temp
                } else if let Some(temp) = air_temperature {
                    //use this as backup
                    temp
                } else {
                    return State::Error;
                };

                if return_temp < config.freeze_protection.min_temperature {
                    State::PrepareHeating((true, Duration::default()))
                } else {
                    if *circulation_duration > config.freeze_protection.check_duration {
                        State::Standby(Duration::default())
                    } else {
                        State::FreezeProtectionCheckCirculation(*circulation_duration + delta_time)
                    }
                }
            }

            State::Error => State::Standby(Duration::default()),
        }
    }
}
