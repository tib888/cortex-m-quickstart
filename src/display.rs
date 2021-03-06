use crate::timing::{Duration, Seconds, WeekTime};
use onewire::temperature::Temperature;

static WEEKDAYS: [&[u8]; 7] = [
    b"Hetfo",
    b"Kedd",
    b"Szerda",
    b"Csutortok",
    b"Pentek",
    b"Szombat",
    b"Vasarnap",
];

pub fn fmt_weekday(n: u8) -> &'static [u8] {
    //assert!(n < 7);
    &(WEEKDAYS[n as usize])
}

pub unsafe fn fmt_nn(n: u8) -> &'static [u8] {
    //assert!(n < 100);
    static mut TEXT: [u8; 2] = [0u8; 2];
    
    TEXT[0] = '0' as u8 + (n / 10u8);
    TEXT[1] = '0' as u8 + (n % 10u8);
    &TEXT
}

pub unsafe fn fmt_duration(duration: &Duration<u32, Seconds>) -> &'static [u8] {
    let (_, min, sec) = duration.to_hms();
    static mut TEXT: [u8; 5] = [0u8; 5];
    let m = min as u8;
    let s = sec as u8;
    
    TEXT[0] = '0' as u8 + (m / 10u8);
    TEXT[1] = '0' as u8 + (m % 10u8);
    TEXT[2] = ':' as u8;
    TEXT[3] = '0' as u8 + (s / 10u8);
    TEXT[4] = '0' as u8 + (s % 10u8);
    &TEXT
}

pub unsafe fn fmt_temp(temp: Temperature) -> &'static [u8] {
    static mut TEXT: [u8; 6] = [0u8; 6];

    TEXT[0] = if temp.is_negative() { '-' } else { ' ' } as u8;

    let t: u8 = temp.whole_degrees() as u8;
    TEXT[1] = '0' as u8 + (t / 10u8);
    TEXT[2] = '0' as u8 + (t % 10u8);
    TEXT[3] = '.' as u8;

    //round fraction to two digits:
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
    TEXT[4] = ROUND_TABLE1[temp.fraction_degrees() as usize];
    TEXT[5] = ROUND_TABLE2[temp.fraction_degrees() as usize];
    &TEXT
}

pub fn print_temp<D: lcd_hal::Display>(
    display: &mut D,
    row: u8,
    prefix: &[u8],
    temp: &Option<Temperature>,
) -> Result<(), D::Error> {
    display.set_position(0, row)?;
    display.print(prefix)?;

    if let Some(temp) = temp {
        display.print(unsafe { fmt_temp(*temp) })?;
    } else {
        display.print(b" -----")?;
    }
    Ok(())
}

pub fn print_nn<D: lcd_hal::Display>(display: &mut D, n: u8) -> Result<(), D::Error> {
    //assert!(n < 100);
    display.print(unsafe { fmt_nn(n) })
}

pub fn print_nnn<D: lcd_hal::Display>(display: &mut D, n: u32) -> Result<(), D::Error> {
    //assert!(n < 1000);
    let a = n / 100;
    display.print_char('0' as u8 + a as u8)?;
    print_nn(display, (n - (a * 100)) as u8)
}

pub fn print_time<D: lcd_hal::Display>(display: &mut D, t: WeekTime) -> Result<(), D::Error> {
    display.print(WEEKDAYS[t.weekday as usize])?;
    display.print_char(' ' as u8)?;
    print_nn(display, t.hour)?;
    display.print_char(':' as u8)?;
    print_nn(display, t.min)
}
