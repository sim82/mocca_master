#![no_main]
#![no_std]
#![feature(slice_fill)]

use crate::hal::prelude::*;
use crate::hal::spi::Spi;
use cortex_m::asm::delay;
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
use micromath::F32Ext;

#[entry]
fn main() -> ! {
    if let Some(mut periphery) = setup() {
        let mut gpioa = periphery.gpioa;
        let mut gpiob = periphery.gpiob;
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
        // for color in Rainbow::step(13) {
        //     data.fill(color);
        //     ws.write(brightness(data.iter().cloned(), 32));
        //     button_wait_debounced(&mut button, &mut periphery.delay);
        // }
        // let cube = Cube::zero();
        let colors = [
            color::RED,
            color::GREEN,
            color::BLUE,
            color::CYAN,
            color::MAGENTA,
            color::YELLOW,
        ];
        let mut canvas = (ws, data);

        // loop {
        //     for i in 2..10 {
        //         data.fill(color::BLACK);
        //         for (cube, color) in hex::CUBE_DIRECTIONS.iter().zip(colors.iter()) {
        //             //set_matrix_oddr((*cube * i).into(), *color, &mut data);
        //             canvas.line(*cube, *cube * i, *color);
        //         }
        //         canvas.apply();
        //         // set_matrix_oddr(Cube::zero().into(), color::BLUE, &mut data);
        //         periphery.delay.delay_ms(200u8);
        //     }
        // }

        // loop {
        //     for i in (-10..10).chain((-10..10).rev()) {
        //         data.fill(color::BLACK);

        //         let a = Vec2::new(-10, i);
        //         let b = Vec2::new(10, -i);

        //         canvas.clear();
        //         canvas.line(a.into(), b.into(), color::CYAN);
        //         canvas.apply();
        //         periphery.delay.delay_ms(200u8);
        //     }
        // }

        loop {
            for i in 0..360 / 6 {
                // canvas.clear();
                canvas.1.iter_mut().for_each(|v| {
                    *v = brightness(core::iter::once(*v), 210).next().unwrap();
                });
                let f = ((i * 6) as f32).to_radians();
                let s = f.sin();
                let c = f.cos();
                // let (sin, cos) = f.sin();
                // f.sin()
                // let v0 = Vec2::new((s * -5f32) as i32, (c * -5f32) as i32);
                let v0 = Cube::zero();
                let v = Vec2::new((s * 15f32) as i32, (c * 15f32) as i32);

                canvas.line(v0.into(), v.into(), color::GREEN);

                canvas.apply();
                periphery.delay.delay_ms(250u8);
                // let v =
            }
        }
        loop {
            data.fill(color::BLACK);

            // let a = Vec2::new(-10, i);
            // let b = Vec2::new(10, -i);

            let lines = [
                (Cube::new(0, 0, 0), Cube::new(5, -5, 0)),
                (Cube::new(5, -5, 0), Cube::new(5, -10, 5)),
                (Cube::new(5, -10, 5), Cube::new(0, -5, 5)),
                (Cube::new(0, -5, 5), Cube::new(0, 0, 0)),
                (Cube::new(0, 0, 0), Cube::new(0, 5, -5)),
            ];

            // for c in hex::CubeLinedraw::new(a.into(), b.into()) {
            //     set_matrix_oddr(c.into(), color::CYAN, &mut data);
            // }
            // ws.write(brightness(data.iter().cloned(), 32));

            for (a, b) in lines.iter() {
                canvas.line(a.into(), b.into(), color::CYAN);
            }
            canvas.apply();

            periphery.delay.delay_ms(200u8);
        }
    }
    unreachable!();
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
