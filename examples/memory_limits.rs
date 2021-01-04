#![no_main]
#![no_std]

use mocca_matrix::prelude::*;
use rt::ExceptionFrame;
use smart_leds::{brightness, SmartLedsWrite, RGB8};
extern crate panic_semihosting;
#[macro_use]
extern crate cortex_m_rt as rt;

// test the memory limits. ~96K of stack and ~1M of flash, just as advertised... (takes ages to flash though)
// the remaining 32K of sram are in a separate segment at 0x10000000
const BIGDATA: [u8; 900 * 1024] = [32u8; 900 * 1024];

fn iter(ws: &mut impl SmartLedsWrite<Color = RGB8, Error = hal::spi::Error>, i: usize) {
    let mut data = [RGB8::default(); NUM_LEDS];

    let mut tmp = i;
    for j in 0..32 {
        if tmp & 0b1 != 0 {
            data[j] = RGB8::new(BIGDATA[i * 1024], 255, 0);
        }
        tmp >>= 1;
    }
    ws.write(brightness(data.iter().cloned(), 32));
    iter(ws, i + 1);
}

#[entry]
fn main() -> ! {
    if let Some((mut ws, mut delay)) = setup_simple() {
        let mut data = [RGB8::default(); NUM_LEDS];
        iter(&mut ws, 0);
    }
    unreachable!();
}
#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
