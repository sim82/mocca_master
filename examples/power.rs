#![no_main]
#![no_std]
use core::convert::Infallible;

use mocca_matrix::prelude::*;
use stm32l4xx_hal as hal;
use ws2812_spi as ws2812;
#[macro_use]
extern crate cortex_m_rt as rt;
use crate::hal::delay::Delay;
use crate::hal::prelude::*;
use crate::hal::spi::Spi;
use crate::hal::stm32;
use crate::rt::entry;
use crate::rt::ExceptionFrame;
use crate::ws2812::Ws2812;
use cortex_m::peripheral::Peripherals;
use smart_leds::{brightness, SmartLedsWrite, RGB8};
extern crate cortex_m_semihosting as sh;
extern crate panic_semihosting;
#[entry]
fn main() -> ! {
    if let (Some(p), Some(cp)) = (stm32::Peripherals::take(), Peripherals::take()) {
        // Constrain clocking registers
        let mut flash = p.FLASH.constrain();
        let mut rcc = p.RCC.constrain();
        let mut pwr = p.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc // full speed (64 & 80MHz) use the 16MHZ HSI osc + PLL (but slower / intermediate values need MSI)
            .cfgr
            .sysclk(80.mhz())
            .pclk1(80.mhz())
            .pclk2(80.mhz())
            .freeze(&mut flash.acr, &mut pwr);

        let mut gpioa = p.GPIOA.split(&mut rcc.ahb2);

        // Get delay provider
        let mut delay = Delay::new(cp.SYST, clocks);

        // Configure pins for SPI
        let (sck, miso, mosi) = cortex_m::interrupt::free(move |cs| {
            (
                gpioa.pa5.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
                gpioa.pa6.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
                gpioa.pa7.into_af5(&mut gpioa.moder, &mut gpioa.afrl),
            )
        });

        // Configure SPI with 3Mhz rate
        let spi = Spi::spi1(
            p.SPI1,
            (sck, miso, mosi),
            ws2812::MODE,
            3_000_000.hz(),
            clocks,
            &mut rcc.apb2,
        );
        let mut gpioc = p.GPIOC.split(&mut rcc.ahb2);
        let button = gpioc
            .pc13
            .into_pull_up_input(&mut gpioc.moder, &mut gpioc.pupdr);
        let mut ws = Ws2812::new(spi);

        enum Mode {
            WhiteAllUp,
            WhiteAddOne,
        }
        match Mode::WhiteAddOne {
            Mode::WhiteAllUp => {
                let colors = [
                    RGB8::new(255, 0, 0),
                    RGB8::new(0, 255, 0),
                    RGB8::new(0, 0, 255),
                ];

                let colors = [10, 32, 64, 96, 128, 160, 192, 200, 224, 255];
                let colors = [255];
                let mut gamma = 15;

                for color in colors.iter().cycle() {
                    let mut data = [RGB8::new(*color, *color, *color); NUM_LEDS];

                    ws.write(brightness(data.iter().cloned(), gamma)).unwrap();

                    while button.is_high().unwrap() {}
                    while button.is_low().unwrap() {}
                    gamma += 16;
                }
            }
            Mode::WhiteAddOne => loop {
                let mut data = [RGB8::default(); NUM_LEDS];
                ws.write(data.iter().cloned()).unwrap();
                while button.is_high().unwrap() {}
                while button.is_low().unwrap() {}
                for i in 0..NUM_LEDS {
                    data[i] = RGB8::new(255, 255, 255);
                    ws.write(data.iter().cloned()).unwrap();
                    // while button.is_high().unwrap() {}

                    button_wait_debounced(&button, &mut delay);
                    // for i in [100, 30, 30, 30, 30].iter() {
                    //     delay.delay_ms(*i as u8);
                    //     if button.is_high().unwrap() {
                    //         break;
                    //     }
                    // }
                    // while button.is_low().unwrap() {}
                }
            },
        }
    }
    unreachable!();
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
