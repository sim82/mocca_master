#![no_main]
#![no_std]
#![feature(slice_fill)]

use crate::hal;
use crate::hal::prelude::*;
use crate::hal::spi::Spi;
use mocca_matrix::prelude::*;
#[macro_use]
extern crate cortex_m_rt as rt;
use rt::ExceptionFrame;
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use ws2812::Ws2812;
use ws2812_spi as ws2812;
extern crate panic_semihosting;

#[entry]
fn main() -> ! {
    if let Some(mut periphery) = setup() {
        let mut gpioa = periphery.gpioa;
        let mut gpioc = periphery.gpioc;
        let (sck, miso, mosi) = cortex_m::interrupt::free(move |cs| {
            (
                gpioa.pa5.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
                gpioa.pa6.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
                gpioa.pa7.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
            )
        });

        // Configure SPI with 3Mhz rate
        let spi = Spi::spi1(
            periphery.spi1,
            (sck, miso, mosi),
            ws2812::MODE,
            3_000_000.hz(),
            periphery.clocks,
            &mut periphery.apb2,
        );
        let mut ws = Ws2812::new(spi);
        let mut button = gpioc
            .pc13
            .into_pull_up_input(&mut gpioc.moder, &mut gpioc.pupdr);

        let mut data = [RGB8::new(0, 0, 0); NUM_LEDS];
        for color in Rainbow::step(13) {
            data.fill(color);
            ws.write(brightness(data.iter().cloned(), 32));
            button_wait_debounced(&mut button, &mut periphery.delay);
        }
    }
    unreachable!();
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
