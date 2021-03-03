#![feature(array_value_iter)]
#![feature(slice_fill)]

use core::convert::Infallible;

use crate::hal::prelude::*;
use crate::hal::spi::Spi;
use crate::hal::stm32;
use crate::{hal::delay::Delay, prelude::*};
// use crate::ws2812::Ws2812;
use arrayvec::ArrayString;
use cortex_m::peripheral::Peripherals;
use embedded_graphics::{
    drawable::Drawable,
    fonts,
    pixelcolor::{self, BinaryColor},
    prelude::{Point, Primitive},
    primitives::Rectangle,
    style::{self, PrimitiveStyleBuilder},
};
use hal::{
    gpio::{Input, PullUp},
    i2c::I2c,
};
use numtoa::NumToA;
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use ssd1306::{
    displaysize::DisplaySize, mode::GraphicsMode, prelude::WriteOnlyDataCommand, I2CDIBuilder,
};
use stm32l4xx_hal as hal;
use ws2812::Ws2812;
use ws2812_spi as ws2812;
// extern crate cortex_m_semihosting as sh;
// extern crate panic_semihosting;
use bitset_core::BitSet;
use core::fmt::Write;
use heapless::consts::*;
pub trait Console {
    fn write(&mut self, t: &str, line: Option<i32>);
}

impl core::fmt::Write for &mut dyn Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s, None);
        Ok(())
    }
}

struct ScrollConsole {}

impl<DI, DSIZE> Console for GraphicsMode<DI, DSIZE>
where
    DSIZE: DisplaySize,
    DI: WriteOnlyDataCommand,
{
    fn write(&mut self, t: &str, line: Option<i32>) {
        // self.clear();
        let style = PrimitiveStyleBuilder::new()
            .stroke_width(1)
            .stroke_color(BinaryColor::Off)
            .fill_color(BinaryColor::Off)
            .build();

        let y = match line {
            Some(l) => l * 8,
            None => 0,
        };

        Rectangle::new(Point::new(0, y), Point::new(127, y + 7))
            .into_styled(style)
            .draw(self)
            .unwrap();
        fonts::Text::new(t, Point::new(0, y))
            .into_styled(style::TextStyle::new(
                fonts::Font6x8,
                pixelcolor::BinaryColor::On,
            ))
            .draw(self)
            .unwrap();
        self.flush().unwrap();
    }
}

pub trait Interface {
    fn console(&mut self) -> &mut dyn Console;
    fn canvas(&mut self) -> &mut dyn Canvas;
}

struct InterfaceImpl<'a> {
    console: &'a mut dyn Console,
    canvas: &'a mut dyn Canvas,
}

impl<'a> Interface for InterfaceImpl<'a> {
    fn console(&mut self) -> &mut dyn Console {
        self.console
    }
    fn canvas(&mut self) -> &mut dyn Canvas {
        self.canvas
    }
}
pub trait Schedule {
    fn get_timing(&self) -> u32;
    fn run(&mut self, interface: &mut dyn Interface);
}

pub fn enter(sched: &mut [&mut dyn Schedule]) -> ! {
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
        // Mode::Fast {
        //     frequency: 400_000.hz(),
        //     duty_cycle: DutyCycle::Ratio2to1,
        // },
        let i2c = I2c::i2c1(p.I2C1, (scl, sda), 1000.khz(), clocks, &mut rcc.apb1r1);

        let interface = I2CDIBuilder::new().init(i2c);
        let mut disp: GraphicsMode<_, _> = ssd1306::Builder::new().connect(interface).into();

        disp.init().unwrap();
        disp.flush().unwrap();

        disp.write("Init!", None);
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

        // let mut ws = Ws2812::new(spi);
        // delay.delay_ms(200u8);
        let mut text = heapless::String::<U32>::new();
        write!(&mut text, "sys: {:?}", clocks.sysclk().0 / 1_000_000).unwrap();
        disp.write(&text, Some(0));
        text.clear();
        write!(&mut text, "pclk1: {:?}", clocks.pclk1().0 / 1_000_000).unwrap();
        disp.write(&text, Some(1));
        text.clear();
        write!(&mut text, "pclk2: {:?}", clocks.pclk2().0 / 1_000_000).unwrap();
        disp.write(&text, Some(2));

        // clocks.pclk1()
        // button_wait_debounced(&button, &mut delay);
        // loop {}

        let mut ws = Ws2812::new(spi);
        let mut data = [RGB8::default(); NUM_LEDS];

        // disp.write("Run!", None);
        let mut os_int = InterfaceImpl {
            console: &mut disp,
            canvas: &mut (ws, data),
        };
        let mut prio = [0u32; 16];
        for (i, s) in sched.iter().enumerate() {
            prio[i] = s.get_timing().max(1);
        }

        loop {
            for (i, s) in sched.iter_mut().enumerate() {
                prio[i] -= 1;
                if prio[i] == 0 {
                    s.run(&mut os_int);
                    prio[i] = s.get_timing().max(1);
                }
            }
        }
    }

    unreachable!();
}

pub mod prelude {
    pub use super::{enter, Console, Interface, Schedule};
}
