#![no_std]
#![feature(min_const_generics)]
use smart_leds::RGB8;

pub mod bitzet;
pub mod effects;
pub mod math;

pub use stm32l4xx_hal as hal;

pub mod setup {

    use core::convert::Infallible;

    use hal::{
        delay::Delay,
        gpio::{gpioa::PA5, gpioc::PC13, Alternate, GpioExt, Input, PullUp},
        prelude::*,
        rcc::{RccExt, APB2},
        spi::Spi,
    };
    use smart_leds::{SmartLedsWrite, RGB8};
    use stm32l4::stm32l4x6::SPI1;
    use stm32l4xx_hal as hal;
    use ws2812::Ws2812;
    use ws2812_spi as ws2812;
    pub fn setup_simple() -> Option<(
        impl SmartLedsWrite<Color = RGB8, Error = hal::spi::Error>,
        Delay,
    )> {
        if let (Some(p), Some(cp)) = (
            hal::stm32::Peripherals::take(),
            cortex_m::peripheral::Peripherals::take(),
        ) {
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
            let mut ws = Ws2812::new(spi);

            Some((ws, delay))
        } else {
            None
        }
    }

    pub struct Periphery {
        pub flash: hal::flash::Parts,
        pub pwr: hal::pwr::Pwr,
        pub clocks: hal::rcc::Clocks,
        pub gpioa: hal::gpio::gpioa::Parts,
        pub gpioc: hal::gpio::gpioc::Parts,
        pub delay: Delay,
        pub spi1: SPI1,
        pub apb2: APB2,
    }

    pub fn setup() -> Option<Periphery> {
        if let (Some(p), Some(cp)) = (
            hal::stm32::Peripherals::take(),
            cortex_m::peripheral::Peripherals::take(),
        ) {
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

            let gpioa = p.GPIOA.split(&mut rcc.ahb2);
            let gpioc = p.GPIOC.split(&mut rcc.ahb2);
            // Get delay provider
            let delay = Delay::new(cp.SYST, clocks);

            Some(Periphery {
                flash,
                pwr,
                gpioa,
                gpioc,
                clocks,
                delay,
                spi1: p.SPI1,
                apb2: rcc.apb2,
            })
        } else {
            None
        }
    }
}

pub mod io {
    use core::convert::Infallible;
    use embedded_hal::prelude::*;
    use stm32l4xx_hal::{delay::Delay, prelude::InputPin};

    pub fn button_wait_debounced<B: InputPin<Error = Infallible>>(button: &B, delay: &mut Delay) {
        const DEBOUNCE_TIME: [u8; 5] = [100, 30, 30, 30, 30];
        while button.is_high().unwrap() {}
        for i in DEBOUNCE_TIME.iter() {
            delay.delay_ms(*i as u8);
            if button.is_high().unwrap() {
                break;
            }
        }
    }
}
pub const NUM_LEDS: usize = 291;
const MATRIX_MAP: [i16; 21 * 19] = [
    291, 291, 291, 291, 291, 291, 291, 291, 0, 1, 2, 3, 4, 5, 6, 7, 291, 291, 291, 291, 291, 291,
    291, 291, 291, 291, 16, 15, 14, 13, 12, 11, 10, 9, 8, 291, 291, 291, 291, 291, 291, 291, 291,
    291, 291, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 291, 291, 291, 291, 291, 291, 291, 291, 37,
    36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 291, 291, 291, 291, 291, 38, 39, 40, 41, 42, 43, 44,
    45, 46, 47, 48, 49, 50, 51, 52, 291, 291, 291, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57,
    56, 55, 54, 53, 291, 291, 291, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84,
    85, 291, 102, 101, 100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 291, 291, 103,
    104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 291, 136, 135,
    134, 133, 132, 131, 130, 129, 128, 127, 126, 125, 124, 123, 122, 121, 120, 291, 291, 137, 138,
    139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 291, 291, 170, 169,
    168, 167, 166, 165, 164, 163, 162, 161, 160, 159, 158, 157, 156, 155, 154, 291, 291, 291, 171,
    172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 291, 291, 204,
    203, 202, 201, 200, 199, 198, 197, 196, 195, 194, 193, 192, 191, 190, 189, 188, 291, 291, 291,
    205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 291, 291,
    237, 236, 235, 234, 233, 232, 231, 230, 229, 228, 227, 226, 225, 224, 223, 222, 291, 291, 291,
    291, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 291, 291, 291,
    291, 291, 291, 291, 263, 262, 261, 260, 259, 258, 257, 256, 255, 254, 253, 291, 291, 291, 291,
    291, 291, 291, 291, 291, 264, 265, 266, 267, 268, 269, 270, 271, 272, 273, 291, 291, 291, 291,
    291, 291, 291, 291, 291, 282, 281, 280, 279, 278, 277, 276, 275, 274, 291, 291, 291, 291, 291,
    291, 291, 291, 291, 291, 291, 283, 284, 285, 286, 287, 288, 289, 290, 291, 291, 291,
];
pub const MATRIX_WIDTH: usize = 19;
pub const MATRIX_HEIGHT: usize = 21;

pub enum Error {
    OutOfBounds,
}

pub fn set_matrix(
    x: usize,
    y: usize,
    color: RGB8,
    data: &mut [RGB8; NUM_LEDS],
) -> Result<i16, Error> {
    if x >= MATRIX_WIDTH || y >= MATRIX_HEIGHT {
        return Err(Error::OutOfBounds);
    }
    let addr = x + y * MATRIX_WIDTH;
    let led = MATRIX_MAP.get(addr).ok_or(Error::OutOfBounds)?;
    let rgb = data.get_mut(*led as usize).ok_or(Error::OutOfBounds)?;
    *rgb = color;
    Ok(*led)
}

pub fn get_matrix(x: usize, y: usize, data: &mut [RGB8; NUM_LEDS]) -> Result<(i16, RGB8), Error> {
    if x >= MATRIX_WIDTH || y >= MATRIX_HEIGHT {
        return Err(Error::OutOfBounds);
    }
    let addr = x + y * MATRIX_WIDTH;
    let led = MATRIX_MAP.get(addr).ok_or(Error::OutOfBounds)?;
    Ok((
        *led,
        data.get(*led as usize).cloned().ok_or(Error::OutOfBounds)?,
    ))
}
pub mod color {
    use smart_leds::RGB8;

    pub struct Rainbow {
        pos: u8,
        step: u8,
    }

    impl Default for Rainbow {
        fn default() -> Self {
            Rainbow { pos: 0, step: 1 }
        }
    }

    impl Rainbow {
        pub fn step(step: u8) -> Self {
            Rainbow { pos: 0, step }
        }
    }

    impl Iterator for Rainbow {
        type Item = RGB8;

        fn next(&mut self) -> Option<Self::Item> {
            let c = wheel(self.pos);
            self.pos = self.pos.overflowing_add(self.step).0;
            Some(c)
        }
    }
    /// Input a value 0 to 255 to get a color value
    /// The colours are a transition r - g - b - back to r.
    pub fn wheel(mut wheel_pos: u8) -> RGB8 {
        wheel_pos = 255 - wheel_pos;
        if wheel_pos < 85 {
            return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
        }
        if wheel_pos < 170 {
            wheel_pos -= 85;
            return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
        }
        wheel_pos -= 170;
        (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
    }
}

pub mod prelude {
    pub use super::{
        color::Rainbow, effects, get_matrix, hal, io::button_wait_debounced, set_matrix,
        setup::setup, setup::setup_simple, setup::Periphery, MATRIX_HEIGHT, MATRIX_WIDTH, NUM_LEDS,
    };
}
