rem run this first and leave the terminal open: 
openocd -f "stlink-v2.cfg" -f "stm32f1x.cfg"

rem gdb port 3333
rem telnet port 4444
rem arm-none-eabi-gdb c:\Eclipse.portable\RustForSTM32\cortex-m-quickstart\target\thumbv7m-none-eabi\debug\libcortex_m_quickstart