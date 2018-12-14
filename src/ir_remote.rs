#[derive(Debug, PartialEq)]
pub enum IrCommands {
    N0,
    N1,
    N2,
    N3,
    N4,
    N5,
    N6,
    N7,
    N8,
    N9,

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

pub fn translate(data: u32) -> IrCommands {
    match data >> 8 {
        0x807F02 => IrCommands::Power,
        0x807FAA => IrCommands::Power_,

        0x807F9A => IrCommands::Set_,
        0x807F1A => IrCommands::TVIn_,
        0x807FEA => IrCommands::VolDown_,
        0x807F6A => IrCommands::VolUp_,

        0x807Fc2 => IrCommands::Red,
        0x807Ff0 => IrCommands::Green,
        0x807F08 => IrCommands::Yellow,
        0x807F18 => IrCommands::Blue,

        0x807F88 => IrCommands::Home,
        0x807F98 => IrCommands::Back,
        0x807F32 => IrCommands::Menu,
        0x807F00 => IrCommands::Click,

        0x807Fc8 => IrCommands::Ok,

        0x807F68 => IrCommands::Up,
        0x807F58 => IrCommands::Down,
        0x807F8A => IrCommands::Left,
        0x807F0A => IrCommands::Right,

        0x807F72 => IrCommands::N1,
        0x807Fb0 => IrCommands::N2,
        0x807F30 => IrCommands::N3,

        0x807F52 => IrCommands::N4,
        0x807F90 => IrCommands::N5,
        0x807F10 => IrCommands::N6,

        0x807F62 => IrCommands::N7,
        0x807Fa0 => IrCommands::N8,
        0x807F20 => IrCommands::N9,

        0x807F82 => IrCommands::Mute,
        0x807F80 => IrCommands::N0,
        0x807F42 => IrCommands::Backspace,

        _ => IrCommands::Unknown,
    }
}
