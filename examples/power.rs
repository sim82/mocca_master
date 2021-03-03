#![no_main]
#![no_std]
use core::convert::Infallible;

use arrayvec::ArrayString;
use hal::i2c::I2c;
use mocca_matrix::{os::Console, prelude::*};
use numtoa::NumToA;
use ssd1306::{mode::GraphicsMode, I2CDIBuilder};
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

fn rgb8_to_power(c: &RGB8) -> u32 {
    let tmp = c.r as u32 + c.g as u32 + c.b as u32;
    tmp * 12 / 255
}

fn estimate_current(data: &[RGB8; NUM_LEDS]) -> [u32; 4] {
    let start0 = 0;
    let size0 = 8 + 9 + 10 + 11 + 15 + 16 + 17;
    let start1 = size0;
    let size1 = 17 + 17 + 17 + 17;
    let start2 = start1 + size1;
    let size2 = 17 + 17 + 17 + 17;
    let start3 = start2 + size2;
    let size3 = 16 + 15 + 11 + 10 + 9 + 8;
    let end3 = start3 + size3;
    assert!(size0 + size1 + size2 + size3 == 291);

    let zones = [start0..start1, start1..start2, start2..start3, start3..end3];

    let mut out = [0; 4];
    for (i, range) in zones.iter().enumerate() {
        out[i] = 78
            + data[range.clone()]
                .iter()
                .map(|c| rgb8_to_power(c))
                .sum::<u32>();
    }

    out
}

#[entry]
fn main() -> ! {
    if let (Some(p), Some(cp)) = (stm32::Peripherals::take(), Peripherals::take()) {
        // Constrain clocking registers
        let mut flash = p.FLASH.constrain();
        let mut rcc = p.RCC.constrain();
        let mut pwr = p.PWR.constrain(&mut rcc.apb1r1);
        let clocks = rcc // full speed (64 & 80MHz) use the 16MHZ HSI osc + PLL (but slower / intermediate values need MSI)
            .cfgr
            .sysclk(64.mhz())
            .pclk1(16.mhz())
            .pclk2(64.mhz())
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

        let mut gpiob = p.GPIOB.split(&mut rcc.ahb2);
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

        let i2c = I2c::i2c1(p.I2C1, (scl, sda), 400.khz(), clocks, &mut rcc.apb1r1);

        let interface = I2CDIBuilder::new().init(i2c);
        let mut disp: GraphicsMode<_, _> = ssd1306::Builder::new().connect(interface).into();

        disp.init().unwrap();
        disp.flush().unwrap();

        disp.write("Init!", None);

        enum Mode {
            WhiteAllUp,
            WhiteAddOne,
            Special,
        }
        match Mode::Special {
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
                    // for i in 86..NUM_LEDS {
                    //     data[i] = RGB8::new(0,0,0);
                    // }

                    ws.write(brightness(data.iter().cloned(), gamma)).unwrap();

                    button_wait_debounced(&button, &mut delay);
                    gamma += 16;
                }
            }
            Mode::WhiteAddOne => loop {
                let mut data = [RGB8::default(); NUM_LEDS];
                ws.write(data.iter().cloned()).unwrap();
                // while button.is_high().unwrap() {}
                // while button.is_low().unwrap() {}
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
            Mode::Special => {
                let mut data = [RGB8::default(); NUM_LEDS];
                ws.write(data.iter().cloned()).unwrap();
                for i in 0..50 {
                    data[i] = RGB8 { r: 0, g: 0, b: 64 };
                }

                let current = estimate_current(&data);

                for (i, c) in current.iter().enumerate() {
                    let mut num_buffer = [0u8; 20];
                    let mut text = ArrayString::<[_; 100]>::new();
                    text.push_str(c.numtoa_str(10, &mut num_buffer));
                    disp.write(&text, Some(i as i32));
                }
                ws.write(data.iter().cloned()).unwrap();
                loop {}
            }
        }
    }
    unreachable!();
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("{:#?}", ef);
}
