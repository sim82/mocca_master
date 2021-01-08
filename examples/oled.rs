#![no_main]
#![no_std]
#![feature(slice_fill)]

use crate::hal::prelude::*;
use crate::hal::spi::Spi;
use cortex_m::{asm::delay, interrupt};
use hal::i2c::I2c;
use mocca_matrix::color;
use mocca_matrix::prelude::*;
use mocca_matrix::{hex, hex::prelude::*, math::Vec2};
#[macro_use]
extern crate cortex_m_rt as rt;
use rt::ExceptionFrame;
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use ws2812::Ws2812;
use ws2812_spi as ws2812;
extern crate panic_semihosting;
use embedded_graphics::{
    fonts::{Font6x8, Text},
    pixelcolor::BinaryColor,
    prelude::*,
    style::TextStyle,
};
use micromath::F32Ext;
use ssd1306::{prelude::*, Builder, I2CDIBuilder};

#[entry]
fn main() -> ! {
    if let Some(mut periphery) = setup() {
        let mut gpiob = periphery.gpiob;

        let mut scl = gpiob
            .pb6
            .into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
        scl.internal_pull_up(&mut gpiob.pupdr, true);
        let scl = scl.into_af4(&mut gpiob.moder, &mut gpiob.afrl);
        let mut sda = gpiob
            .pb7
            .into_open_drain_output(&mut gpiob.moder, &mut gpiob.otyper);
        sda.internal_pull_up(&mut gpiob.pupdr, true);
        let sda = sda.into_af4(&mut gpiob.moder, &mut gpiob.afrl);

        let mut i2c = I2c::i2c1(
            periphery.i2c1,
            (scl, sda),
            100.khz(),
            periphery.clocks,
            &mut periphery.apb1r1,
        );

        // let mut readbuf = [0u8; 1];
        // i2c.write_read(0x38u8, &[0xafu8], &mut readbuf[..1]);

        // loop {}

        let interface = I2CDIBuilder::new().init(i2c);
        let mut disp: GraphicsMode<_, _> = Builder::new()
            // .with_size(DisplaySize::Display128x64NoOffset)
            .connect(interface)
            .into();

        disp.init().unwrap();
        disp.flush().unwrap();

        Text::new("Hello world!", Point::zero())
            .into_styled(TextStyle::new(Font6x8, BinaryColor::On))
            .draw(&mut disp)
            .unwrap();

        Text::new("Hello Rust!", Point::new(0, 16))
            .into_styled(TextStyle::new(Font6x8, BinaryColor::On))
            .draw(&mut disp)
            .unwrap();

        disp.flush().unwrap();

        loop {}
    }
    unreachable!();
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
