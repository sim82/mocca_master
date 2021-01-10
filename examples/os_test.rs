#![no_main]
#![no_std]
#![feature(slice_fill)]

use arrayvec::ArrayString;

use cortex_m::interrupt;
use mocca_matrix::{color, hex::prelude::*, math::Vec2, os::Interface};
use mocca_matrix::{os::prelude::*, prelude::*};

#[macro_use]
extern crate cortex_m_rt as rt;
use micromath::F32Ext;
use numtoa::NumToA;
use rt::ExceptionFrame;
use smart_leds::brightness;
extern crate panic_semihosting;
use core::fmt::Write;
use heapless::consts::*;
#[macro_use]
use heapless::String;
struct TestSched {
    i: i32,
    timing: u32,
    line: i32,
}

impl Schedule for TestSched {
    fn run(&mut self, interface: &mut dyn Interface) {
        // let mut text = String::<U32>::new();
        let text: String<U32> = "012345678901234567890".into();

        for i in (0..8) {
            interface.console().write(&text, Some(i));
        }

        // write!(&mut text, "num: {}", self.i).unwrap();

        // // let mut num_buffer = [0u8; 20];
        // // // let mut text = ArrayString::<[_; 100]>::new();
        // // text.push_str("num: ");
        // // // text.push_str(self.i.numtoa_str(10, &mut num_buffer));
        // // // text.push_str(self.i.into());
        // interface.console().write(&text, Some(self.line));

        // write!(interface.console(), "meeep {}", self.i).unwrap();

        self.i += 1;
    }
    fn get_timing(&self) -> u32 {
        self.timing
    }
}

struct Radar {
    colors: Rainbow,
    i: u32,
}

impl Schedule for Radar {
    fn get_timing(&self) -> u32 {
        1
    }

    fn run(&mut self, interface: &mut dyn Interface) {
        let i = self.i % (360 / 6);

        let canvas = interface.canvas();
        canvas.data().iter_mut().for_each(|v| {
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

        canvas.line(v0.into(), v.into(), self.colors.next().unwrap());

        canvas.apply();
        self.i += 1;
        // periphery.delay.delay_ms(250u8);
    }
}

#[entry]
fn main() -> ! {
    mocca_matrix::os::enter(&mut [
        // &mut TestSched {
        //     i: 0,
        //     timing: 4,
        //     line: 0,
        // },
        // &mut TestSched {
        //     i: 0,
        //     timing: 20,
        //     line: 1,
        // },
        &mut Radar {
            i: 0,
            colors: Rainbow::step(5),
        },
    ])
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
