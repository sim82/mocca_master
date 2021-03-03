#![no_main]
#![no_std]
#![feature(array_value_iter)]
#![feature(slice_fill)]

use core::convert::Infallible;

use arrayvec::ArrayString;
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
use mocca_matrix::{bitzet::Bitzet, prelude::*};
use numtoa::NumToA;
use ssd1306::{
    displaysize::DisplaySize, mode::GraphicsMode, prelude::WriteOnlyDataCommand, I2CDIBuilder,
};
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
use bitset_core::BitSet;
use mocca_matrix::math::Vec2;
use mocca_matrix::os::Console;

// trait Console {
//     fn write(&mut self, t: &str);
// }

// impl<DI, DSIZE> Console for GraphicsMode<DI, DSIZE>
// where
//     DSIZE: DisplaySize,
//     DI: WriteOnlyDataCommand,
// {
//     fn write(&mut self, t: &str) {
//         // self.clear();
//         let style = PrimitiveStyleBuilder::new()
//             .stroke_width(1)
//             .stroke_color(BinaryColor::Off)
//             .fill_color(BinaryColor::Off)
//             .build();

//         Rectangle::new(Point::new(0, 0), Point::new(127, 15))
//             .into_styled(style)
//             .draw(self)
//             .unwrap();
//         fonts::Text::new(t, Point::zero())
//             .into_styled(style::TextStyle::new(
//                 fonts::Font6x8,
//                 pixelcolor::BinaryColor::On,
//             ))
//             .draw(self)
//             .unwrap();
//         self.flush().unwrap();
//     }
// }

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
        let mut data = [RGB8::new(0, 0, 0); NUM_LEDS];
        let mut rainbow = Rainbow::step(13);
        for _ in 0..1 {
            effects::kitt(&mut ws, &mut rainbow, &mut data);
        }
        delay.delay_ms(200u8);
        ws.write(brightness(data.iter().cloned(), 0)).unwrap();
        // button_wait_debounced(&button, &mut delay);

        disp.write("Run!", None);
        run(&mut ws, &mut delay, &button, &mut disp);
        let mut data = [RGB8::new(255, 0, 0); NUM_LEDS];
        ws.write(brightness(data.iter().cloned(), 32)).unwrap();
    }
    unreachable!();
}

fn adjacent(v: Vec2) -> [Vec2; 6] {
    let xshift = v.y.abs() % 2;
    let mut d = [
        Vec2::new(1, 0),
        Vec2::new(-1, 0),
        Vec2::new(-1 + xshift, 1),
        Vec2::new(0 + xshift, 1),
        Vec2::new(-1 + xshift, -1),
        Vec2::new(0 + xshift, -1),
    ];

    d.iter_mut().for_each(|f| *f = *f + v);
    d
}
#[test]
fn test_hex() {
    println!(
        "{:?}",
        std::array::IntoIter::new(adjacent(Vec2(0, 0))).collect::<Vec<_>>()
    );
    println!("{:?}", adjacent(Vec2(0, 1)));
    println!("{:?}", adjacent(Vec2(0, -1)));
}
fn run<WS: SmartLedsWrite<Color = RGB8, Error = hal::spi::Error>>(
    ws: &mut WS,
    delay: &mut Delay,
    button: &dyn InputPin<Error = Infallible>,
    console: &mut Console,
) -> Result<(), mocca_matrix::Error> {
    type BitzetN = Bitzet<128>;
    let mut data = [RGB8::default(); NUM_LEDS];
    let mut black = BitzetN::new();
    for (i, line) in input().iter().enumerate() {
        let mut c = line.chars();
        let mut x = 0i32;
        let mut y = 0i32;

        // data[i % NUM_LEDS] = RGB8::new(0, 255, 0);
        let mut prev = None;
        fn reset_prev(prev: Option<(i16, RGB8)>, data: &mut [RGB8]) {
            if let Some((led, color)) = prev {
                data[led as usize] = color;
            }
        }
        loop {
            match c.next() {
                Some('e') => x += 1,
                Some('w') => x -= 1,
                Some('s') => match c.next() {
                    Some('e') => {
                        x += (y.abs() % 2);
                        y += 1
                    }
                    Some('w') => {
                        y += 1;
                        x -= (y.abs() % 2);
                    }
                    _ => break,
                },
                Some('n') => match c.next() {
                    Some('e') => {
                        x += (y.abs() % 2);
                        y -= 1
                    }
                    Some('w') => {
                        y -= 1;
                        x -= y.abs() % 2;
                    }
                    _ => break,
                },
                None => break,

                _ => break,
            }

            reset_prev(prev, &mut data);
            prev = get_matrix((x + 10) as usize, (y + 10) as usize, &mut data).ok();
            set_matrix(
                (x + 10) as usize,
                (y + 10) as usize,
                RGB8::new(0, 255, 0),
                &mut data,
            );
            ws.write(brightness(data.iter().cloned(), 32)).unwrap();
            delay.delay_ms(8u8);
        }
        reset_prev(prev, &mut data);

        if black.contains(&Vec2 { x, y }) {
            black.remove(&Vec2 { x, y });
        } else {
            black.insert(Vec2 { x, y });
        }
        set_matrix(
            (x + 10) as usize,
            (y + 10) as usize,
            RGB8::new(0, 0, 255),
            &mut data,
        );
        ws.write(brightness(data.iter().cloned(), 32)).unwrap();
    }
    {
        let mut rainbow = Rainbow::step(3);
        for _ in 0..100 {
            let c = rainbow.next().unwrap();
            data.iter_mut().for_each(|v| {
                if v.r != 0 || v.g != 0 || v.b != 0 {
                    *v = c
                }
            });
            ws.write(brightness(data.iter().cloned(), 32)).unwrap();
        }
    }
    // while button.is_high().unwrap() {}

    let mut i: usize = 0;
    let mut keep_on = [0u32; NUM_LEDS / 32 + 1];
    let mut rainbow = Rainbow::step(7);
    loop {
        let warp_mode = false; //button.is_low().unwrap();
        let hold_mode = false; //button.is_low().unwrap();
        if i % 100 == 0 || warp_mode {
            let mut black_new = Bitzet::new();

            for v in black.iter() {
                let n = adjacent(v).iter().filter(|v| black.contains(*v)).count();
                if (1..=2).contains(&n) {
                    black_new.insert(v);
                }
            }

            let white = black
                .iter()
                .flat_map(|v| core::array::IntoIter::new(adjacent(v)))
                .collect::<BitzetN>();

            let white = white.difference(&black);
            for v in white.iter() {
                let n = adjacent(v).iter().filter(|v| black.contains(*v)).count();
                if n == 2 && v.x.abs() < 15 && v.y.abs() < 15 {
                    black_new.insert(v);
                }
            }

            if !hold_mode {
                black = black_new;

                {
                    let mut num_buffer = [0u8; 20];
                    let mut text = ArrayString::<[_; 100]>::new();
                    text.push_str("num: ");
                    text.push_str(black.len().numtoa_str(10, &mut num_buffer));
                    console.write(&text, Some(0));
                }
            }
            keep_on.fill(0);
            if warp_mode {
                data.fill(RGB8::default());
            }
            for v in black.iter() {
                if let Ok(addr) = set_matrix(
                    (v.x + 10) as usize,
                    (v.y + 10) as usize,
                    rainbow.next().unwrap(),
                    &mut data,
                ) {
                    keep_on.bit_set(addr as usize);
                }
            }
        }
        i = i.overflowing_add(1).0;

        if !warp_mode {
            for i in 0..NUM_LEDS {
                if !keep_on.bit_test(i) {
                    let v = &mut data[i];
                    // let old = [v.clone(); 1];
                    *v = brightness(core::iter::once(*v), 220).next().unwrap();
                }
            }
        }

        let current = mocca_matrix::estimate_current(&data);

        for (i, c) in current.iter().enumerate() {
            let mut num_buffer = [0u8; 20];
            let mut text = ArrayString::<[_; 100]>::new();
            text.push_str(c.numtoa_str(10, &mut num_buffer));
            console.write(&text, Some((i + 1) as i32));
        }

        ws.write(brightness(data.iter().cloned(), 255)).unwrap();
    }
    Ok(())
}

fn input() -> &'static [&'static str] {
    &[
        "eeeee",
        "wwwwwwswsw",
        "neneeseswswswee",
        "w",
        "wwwwwwswswee",
        "wnwnwwswswsese",
        "wwwwwwnenwwsw",
        "wnwnwwswsw",
        "eeeeene",
        "eeeeese",
        "wnwnw",
        "wnwnww",
        "neneesesw",
        "wwwwwwnenww",
        "ne",
        "wnwnwwswswse",
        "wnwnww",
        "neneeseswswswe",
        "wwwwww",
        "eeeeesesw",
        "nene",
        "wwwwwwswswe",
        "neneeseswsw",
        "wwwwwwne",
        "eeeeenenw",
        "wnwnwwsw",
        "neneese",
        "wnwnwwswswsesee",
        "wnwnwwswswseseene",
        "wnw",
        "wwwwwwnenw",
        "wwwwwwsw",
        "nenee",
        "neneeseswswsw",
        //   //0
        //   "w",
        //   "wnw",
        //   "wnwnw",
        //   "wnwnww",
        //   "wnwnww",
        //   "wnwnwwsw",
        //   "wnwnwwswsw",
        //   "wnwnwwswswse",
        //   "wnwnwwswswsese",
        //   "wnwnwwswswsesee",
        //   "wnwnwwswswseseene",
        //   //2
        //   "wwwwww",
        //   "wwwwwwsw",
        //   "wwwwwwne",
        //   "wwwwwwswsw",
        //   "wwwwwwnenw",
        //   "wwwwwwswswe",
        //   "wwwwwwnenww",
        //   "wwwwwwswswee",
        //   "wwwwwwnenwwsw",
        //   //2
        //   "ne",
        //   "nene",
        //   "nenee",
        //   "neneese",
        //   "neneesesw",
        //   "neneeseswsw",
        //   "neneeseswswsw",
        //   "neneeseswswswe",
        //   "neneeseswswswee",
        //   //1
        //   "eeeee",
        //   "eeeeene",
        //   "eeeeenenw",
        //   "eeeeese",
        //   "eeeeesesw",

        // "swswswswswnwswswswsweswsesw",
        // "wneswseseneswnweneswwswseswwnwseswswe",
        // "ewswswswwswnwnwsweswwwwwswwwswwsw",
        // "senwseseswseseswswesewseseseseseswsese",
        // "seswnwseseswseseswswseseswswseswesese",
        // "swnwnenwnenwnwnwswnwnwnwnwnwnwnwenwnwnw",
        // "seseeseneswnwseseneeeseeswsewseseese",
        // "senenwnwnewnwnwnwnenwnenwnwnwnenwswnenw",
        // "eneneneweeneneeneneneeneneenesew",
        // "nenesenewwenwseseseseswsesewneseweswse",
        // "swswweswswswswswswswseswnenwswswsweswsw",
        // "nwswnwnwnwnwnwnenenenwnwnw",
        // "seseeneseeesenesewseswsewseeeese",
        // "wwseswwnwneswsewswwswswswwswswswswne",
        // "wwewwnwwwswsenwnewnwwnwsewwww",
        // "wnwnwnwenwnwnwnenwnweswnwnwwswnwnwnw",
        // "neeneneswnenenenenwnenenwnenenenenewnwe",
        // "nenwsenwnwnwnwnwswswnwnwnwenwnwnwnwnwnw",
        // "swenwsenweenwswnwswseseeswswesenwne",
        // "ewseswsewswneewneneneeneseneeswnwsw",
        // "nwswnwswewswswswesesweswswse",
        // "swswswswenwswswswwswneswneswswswsesww",
        // "seswswseseswswsenwswse",
        // "wwwwwwwswsewnewwnwwwwwew",
        // "swwwswewswswswswswnwswswswswseswsww",
        // "sesewsewneseneseseseswswseeseseswnesw",
        // "wwswswwwwswwewwswwwsewnwnww",
        // "nwnenwnenenwnenwnwnwsenwnwne",
        // "neewwwneswseseeeneeseesesesewee",
        // "senwneeneneenewneneneneneneneswnenenene",
        // "swswswneswwswswwswswsewneswesw",
        // "nenenenenwnenesewnenenenenenenwnwnenwne",
        // "nwnwnwsesenwnwwsewnwnenwnewnwnwwwnwnw",
        // "nwnenwwnweswseseeswenenwweneneeswswsw",
        // "eeesweeeenweseseweeneeseeese",
        // "swswseseswseswnwseseswswswseseseneswswsese",
        // "swnwnwewnewwwwnwnwwnwwswnwse",
        // "sesenwnenenwnenwnwnwnenwnewnwnwnw",
        // "neswneneeneneenenenenewnwneneswnenwewne",
        // "nwnenwswnwenwnwnwnwnwnwnwswnwnwnwnwnwnwnw",
        // "nwwwewwneewswneswnwwwwwsewsew",
        // "wswswswswswweswwwwswseswswswneswswe",
        // "wnwwnwnwnewnwnwnwwnwwsenwwnwnwwnw",
        // "wswwwswswwwwnesew",
        // "nwnwneenenwnwswnwnwnwswnwsenwnwwenenenwnw",
        // "swsewseseswswseswneseseswseseswseseseese",
        // "swseswseswnwsewneweseseseeswsesesesew",
        // "sweswwwneswswwswesenenwsweswswswswse",
        // "wswseneeseweeeeweseesenwnwseew",
        // "wwnwnwwwnwnwnwnwwnwnwnwnwswwnwwe",
        // "swnwnwwseneneneneneneswswswnewenwsese",
        // "seseseseseesesenwseseneesewswseswnwnw",
        // "neswswsenwesenesenesenesesewwneswnwswnwne",
        // "nesesesenenwnwnwnenwnwswnwneenewnenwne",
        // "neneneneneeneenewseewnenesenenenenene",
        // "eseseseseneseeseseesenwseseseseseseesww",
        // "neeenenesweeeeeeweneeeeeene",
        // "enwwwwwsewwwnwnwwnwwwwwnwnwnw",
        // "neseeeeseseseeseeseneeseseseenwwsw",
        // "enenesewneenenenwseseneeweeswenwene",
        // "wneneseneneneenenenenenwnenenenenenene",
        // "wnwneeswewnwnwseswswsweswsenewswsw",
        // "eseeswsenwenweseseeseseseesenesesese",
        // "wswswseseseseneenesesesewsesenewsew",
        // "wnwwwwwwewwswwwwwwwswsww",
        // "swenewwwseswwnesesenewneswseneswswswsw",
        // "eswseseseseesewseseneseenwsewseee",
        // "swneneswneneneneeneeenwneeenenwwenene",
        // "neswnenesenenwnenenenenenenenenenenenwnene",
        // "sesesesesewsesesweseseseseseseesesenee",
        // "nwnwnwnwnwnwswnwnesewnwnwnwwnwnwnwnenw",
        // "eenesewwwwnwnwwwwnwwwsw",
        // "swnenwwswswswneseeseneswneseswwseswsw",
        // "neneenenwnenenwnwnwseswnwnwnenwnenenenenw",
        // "neenenwneneseneeneneeneneeeeneswnee",
        // "swswswswneseswwswswseswswweneenwnew",
        // "neseewsesesewswsese",
        // "enenweneeneneeeeneeswswneneeeneenw",
        // "seeeeneeeenwseewswenewnwseesew",
        // "sweeeneesenweswewsweseeseenenwee",
        // "seesesewswenenwnweswsewseneenwsesw",
        // "swwswwswwwneswwsww",
        // "wwwwwwwwwwwwewwwweww",
        // "neneneseneeneneeswneenesenwnewnenwe",
        // "nwnwnwnwnwnenwnwnwnwsenwnwnwwnwnwnwnwnw",
        // "eeeeeseseeenweeeesweeenee",
        // "nwnenwnwneneseneswnenwnenwwnwnwnenenwnwnw",
        // "sewswswwsewwneesewnewnewne",
        // "nwnwwnweswnwneswswnwnwnwnenwnweswenwe",
        // "nwneenenenenwneneneeneseeneneswnenenene",
        // "swwwwswswswswnewwwwwsw",
        // "neneneneeneeneeneneeewnwneswneneesene",
        // "nenenewnenenenenenenewnenwnwneesenene",
        // "swnwswswwwswswwweswnewswswwswnwswsee",
        // "neseseeseseeseseseeeseseewesewse",
        // "nwnenenesesenwnwswnwwswwnwnwnenwnwsew",
        // "seesesewseneseseseseseseswsesesenwswsese",
        // "nwsenwwnenwneseswwwnwwnewesewwswnw",
        // "sesesesenesesesesesewsewsesesesesesesesee",
        // "seswwnwseeswnwwneeneseweeseenee",
        // "nwseenwnwnwnewswnw",
        // "nwnwnenwsenwnwnwnwnww",
        // "wnwwwnwenwnwwsenewnwnwnwwsewwnenw",
        // "sesenwsenwseswseseseseeewsenesesesesese",
        // "seeneeeeseeenwnwsweeeeneeene",
        // "eeeeseeeeswneeneeneesweenew",
        // "swseneswswnwseswswseswswswswneswswswsesw",
        // "nesesweneseseneeweesenwnwseeswnwswse",
        // "senwswnenenwnwneeesw",
        // "nweeseeenweesweeeeeseeswese",
        // "sweneenwneeneeneesenwnwnwnesweswswnee",
        // "swswnwewswnwswwwwswwswwwewswswswe",
        // "swswswsenwswwswsweswswnwsweswswesesw",
        // "swswsesesenenwewseseswseeswseseswsesesw",
        // "seswnewewswnenwenwnenenenenenenwwene",
        // "wnwswswwwsewnwnwswweseswsenewwww",
        // "wswswweswwwnwsewwswswwswsw",
        // "swswseswsweswswswswswswswswswswnwswswsw",
        // "nenwnewnenenenwnenenwnwnenwneneneenese",
        // "seswswswswseneswsesw",
        // "neseneneenewsenwwnwnwwswwnwwee",
        // "swswswwswswswwnwnewwwneeneswwnwse",
        // "nwswwwwnwswesewsenwseswwswwwwwnew",
        // "wsenweeeseeeseeseeeeeeeseese",
        // "seseseseseseeswseesesesenesesewnesese",
        // "nenesesewsenwneseseseneseswseswwswsesese",
        // "sewseseewsenenweewswseseneeswsee",
        // "swseewnwsenwnenwswneswnwwswnesenwnewnw",
        // "neeeewseseneswnenwneneweneneseseew",
        // "nenwnenwswneneenenwnesesenewneneswnenwnw",
        // "nenewnenesweneneenesenenenenenenenwneee",
        // "wnwsenwnwsenenenesesesenesewnwnwnwnwnenw",
        // "nwswswswnewnwsweseswnesenweswenwesww",
        // "nwnwnenesenwnwwnwneswnenwnwnwnwnwenwnw",
        // "seswseseswswneswnwswseswsesesenwswswswswsw",
        // "nenwnwsenwnwnwnenwnenenenwsenwwnwnwnenwnwne",
        // "eswwswswwswswswswswswwsww",
        // "nenesenwenwsweesweeneenw",
        // "nenenesenenenenwnwnenenewnenwswnwnenenwne",
        // "eeseeeneseewneeeeseeweeeese",
        // "swswwseseswsenwseeswswswsesesw",
        // "nwnwsenwsenenenwswseswsewseswswwnwswnew",
        // "nwwnwnwnwenwnwwwnwwnwwwnwnwwwsw",
        // "wewnwwswswnwwnwnwnwnwewswnwesenwwne",
        // "nenenwnwnwnwenewenwsenwnenenwnwnewnwne",
        // "seseseenwseseseseseswwseswnwsee",
        // "swseswseseseswsesesesenwseseswsese",
        // "swswneseswseswwswswswseswsweswswswswswsw",
        // "seneneeseseswseseswsenwseeseseswsesee",
        // "nwneneseneneneneneenesenewseneewnene",
        // "nwwnenwnenenwnwsenwnwsenwnwnwnwnenenenw",
        // "wwnwwsewwwwwsewwwnwnwnwnwwnese",
        // "seseswnwseswseswsesesenwsesesese",
        // "wnweswsenwnwswnenwnwnwswwsewnenwnwwnw",
        // "wseswnwseswswseswswsenwswswsesene",
        // "neeeneenwneneeeeneeesweenenweesw",
        // "wwwnwnwwwwwwwwwnwwwwesew",
        // "neseeneneneneneneneneneneenwnenwneneswnene",
        // "neeeesenweeswenewne",
        // "nwswnwenwwwwnwswwwwwwnwnwenwnwe",
        // "wwwwswwwwwwswwwwwewsw",
        // "eeseeswswswneeeweewneneneenenene",
        // "swswswesesesenwswseswnese",
        // "sesewswseseseswseseseseseseseseseswnese",
        // "neeeeeneeweeeesweeenenwe",
        // "nwwnesenwneneswenenenenenwnwnenwnwwnene",
        // "neeenwneneswneneneswneeeneswneene",
        // "nwwewwnwnwnwswwswwwnwwwwwwnew",
        // "nenwnwnwwswnwwnwnenwesee",
        // "wswswswswwseswswswswnewswswnwswwswsww",
        // "neneneneswsenenenwne",
        // "neeeweeeeeeeseneneneneeesewe",
        // "nwswnwnwnwnwenwneeswnwnwwnwnw",
        // "swsesenwseswseseswwseneseneseseesesesese",
        // "nweeweeeeeseeseeswseeee",
        // "nwsesesesesewesenwnenwswneeseneswsesw",
        // "nwneneenenewsweeneneneneneneeewnene",
        // "swseeseneswwneswnwwswneewnesw",
        // "neseseswswseswseswseswsenwsweseswsesesese",
        // "seswswwswwwwewswswwnewwwswswnww",
        // "senwseesesesesesesesesesesesenwsenwsese",
        // "seswnwsesesewseseneseseseesenwsesesesewse",
        // "neeenwnesweenwneeeeeeeeeneswene",
        // "seenwsenweswnwswwnwnwnwnwnewnwswenw",
        // "swswswswswswswswswswwswswseswne",
        // "enwswswnwswneswsewnwswseneseenwwnwsw",
        // "swneswnenenwnwneeeneneneeswneswnenesew",
        // "nwswnwwswwseewseewsenenwnwwwnwwwse",
        // "eeeseesenesesewseeseswseneesesesese",
        // "swnewswseswwwnwwewew",
        // "eeeeeeseeeeeswnwe",
        // "nenenenenwnwneeneswwneeneneneswnenenesw",
        // "eswwswswswsweswnw",
        // "newnwsenwnwnenwnwenwnenenwnwneswnwwswnene",
        // "eeneeneneeeneeweeswneeneseeewne",
        // "nwwnenewenenwnwnwnwe",
        // "neneswwneswneneneswene",
        // "swwneneweneeswwneenesenwnwenenese",
        // "seesewsesesesesenesesesesewsesesesesese",
        // "nesenwesesesenwwswsesewesesesesesesesese",
        // "nwsenwneeweenweswnwnwnwwwenwwne",
        // "seswswswnwswseswswswseswenwswswswswnesw",
        // "swnwsesweswswseswwswswwseeswswsenesesw",
        // "ewseseneeseeeneeenwnweeesenwne",
        // "neswswswswswswswswseseswwenwswnwswswsw",
        // "nwswswwseeswswswswswswswswswswswswswne",
        // "nenwnenenenenwnenenenewnenenwneenwnw",
        // "senweeeenwseeseeenwseeseseeeese",
        // "seseseswseseswswseseseneneseseswswwseswswse",
        // "swswwswwswwswswswswwwwswwswwe",
        // "sesesewseswseseseseseseseesesesese",
        // "nwwnwnwnwnwenwwnwnwnwwnwnw",
        // "nwnwnwnwnewnenwnenwnenwnesenenwnenenwnw",
        // "nwwseneesenwwswwneseenwseswwwnwe",
        // "enwnwswsesenwswsesweeseseseseewnwswse",
        // "eeeeseseeesweneeeewewwse",
        // "ewneneeseeneseswswnwswwseswesesene",
        // "eeeneeeneeweeneeeeeeweese",
        // "nwnesenwnwnewnwnenwnenwnwneeswnwnwneswe",
        // "swseweeeseseneeeeseneseseeneeew",
        // "seewnenenwenesesewnwnwnwseswe",
        // "nesenewsesweseseswwsesewneswswswese",
        // "neenenesenwneeneeneswswneneenenenene",
        // "wneswsenenwsewseeswseswswnesewswnese",
        // "senwnwnenenewwwnenwenwnenwseenenenw",
        // "nwwwsewneswswwnenwswnewnenwseesenenwnw",
        // "eneneneeneseswweeneeewnweneee",
        // "seenenwswswswnwnwwseneneseeeeeswsw",
        // "nenenesenewnenenwneneneneneneenenenenene",
        // "wswwwwwwswwswwnwwswwswewwew",
        // "neenweneneseeneswwswnew",
        // "eeeenweeseneeneeeeeneeswee",
        // "nwsesewsesesewnwseswneseseesesesenwse",
        // "wnwnenenenenwwesweeneneswsenwnwswsene",
        // "swswwswswswswswswwsweneswnwwnwswesw",
        // "wwewwwwnwww",
        // "eeeeneswsweneenwnenwneneneeneene",
        // "sewseswswswswswseseswseewnwnenwsw",
        // "neeneseweeneeneesweeneenweese",
        // "sweeeeeeneeeeeeee",
        // "wnwnwwnenewwnwnwwsewswwwwnwnwwnww",
        // "seswwseseseswnwswseeswswswnwseswseswswse",
        // "eseswnenwweeswnwwneeswnese",
        // "sesesewnewwwwneneww",
        // "nweneesweneesenewnenw",
        // "eesesesenwseseeseseseseseese",
        // "neswswnewenenenenenenwenenenwne",
        // "swnenwnwnwsenwwnwwwneewwnwwwnenwwse",
        // "seswswsesewweneenenwsesewswsw",
        // "seeseeeswenwswnweeeswnweeeeee",
        // "sweswneswwwwswne",
        // "seseswseswswseneswswseswseswneswswseswsese",
        // "wnwwnwnwnwwwwwnwwwwnwenww",
        // "wswseswswswswsweseswswnwseswneswswswsw",
        // "enwwnwnwwseeesewnww",
        // "nwnwsweseswswswwswswseswswweswnwswe",
        // "swseswswswwwswswnewsw",
        // "wswnewnewnewenwsewwwseseswnwnwwse",
        // "eseneseseeesesweesesewseseesesewse",
        // "neewseeseeeswswsesesenenw",
        // "newneweenenesewseewwnwwsenesesee",
        // "wnenwwswwswswnwnweeenwneswseenwnwnw",
        // "swnwnwnwnwenwnenwnwnwnwnwnw",
        // "neneesewnweseeneeneseewsewnwswne",
        // "wwwwwenwwwwwwswwwwwnwsew",
        // "seesweneseeseweeseneseeseseeee",
        // "nwnwnwsewwnwwnwnwnwnwnwnwnwnwnwneswwse",
        // "newnwswnwwnwnwnwnwwwnwnwnwwwnwnwenw",
        // "seweeeseeeseeseeeeeseswnweseene",
        // "eseseeeeeswesesweeseneeesenesee",
        // "eseswwsesenwseseswseseseeswsenese",
        // "wnwnwnwnwnenwnwenenw",
        // "neneenesweswsenwseneneswnenwwwneswneww",
        // "wswneswewswseswswswswwsw",
        // "seseeneeseswsewneswnwseneswswneenwnee",
        // "nwwnesenwsenwnenwnwnesewnwswswnwneswwnw",
        // "enweneeeneeneneeneneswseneseenwne",
        // "nwnenwnenenwnwnwsenwnwnewwnwnenwene",
        // "wwnwneswwewnwweswwneseswwswnesew",
        // "enewneewsewenenenenesenenewene",
        // "nwsenwnwnwnenenenenenenenwnwnwnenw",
        // "nenenenenenwnwneneneswne",
        // "enwneseeneneneneswwneeneneswe",
        // "swswwswsewsewwwwneeeswwwswnwww",
        // "swnwseenenenwenwnenwnwwnwswneswnwnwnenw",
        // "wswswwswwwswswswswswswwswweswnwsw",
        // "seseswwneswwswswswswswswneswswswswswsww",
        // "seseswnwnwnwnwnwnwnwnwwewnwnw",
        // "wwnwnwwwnwnewnwsewwnwnwnwnwnwewsw",
        // "wseswnwwneswnenewwseewseswswwwne",
        // "wswwwwwwwwwnwewwwnw",
        // "senwswseneseseseswswswsweswswseswseswsw",
        // "wwwswwwswwwsewnwswwnewwswswswsw",
        // "swseswswwswseswswseswseseswseswne",
        // "ewswswsweswswnewswseneseswseswsenwnwe",
        // "wwwwwsenwwwwwwwwwwne",
        // "nwsewwwwsewnwwwnwne",
        // "swswswswswnewswwwwwswswswswswsw",
        // "nwwseenwsenenwwnwnwnwnwenwwnwnwwsw",
        // "eswseswswsenenenenwswseswswswseswsenwswse",
        // "wswwswwswwsewwwswswswwnwwnene",
        // "sesesesesesenewwswwseneseseeswsesesese",
        // "swnwnwnwnwnwnwnwnenwnwnwenenwnwnwwnwnwnw",
        // "wneenenwneenwwnenwnwnwneneneneeneswnw",
        // "swwwwwwwwwwwwwwwnewseww",
        // "senwneswseswsesenwsesee",
        // "neswwseseeswseseswswsesww",
        // "nwswseswseseswseneeseseneseseseswseswnesw",
        // "eneeeneneneewenenenenenweenesene",
        // "wnwwnenenwnenwswnwnenweenwnwnenese",
        // "wswseswswneswswswnweswswsweseseswsene",
        // "senwwseneseeesewnesewswsewsesenwnese",
        // "esenesesesesesesesesesesewsesesewsesw",
        // "neeneneseswnwnesenwwnwnwswnenwne",
        // "neenenwnenewneswnewneneneenenenenenwe",
        // "swseswswswswseswswnwswsweswsesesesesw",
        // "nwenwsewnwnenenwnwwnwnwnwswnwnwnwnwwnw",
        // "wsenwwnwswneweswwweewnewesene",
        // "swwnwwswwwnwnwewewewwwnwnwnww",
        // "wsewwnwswsweswswswswswswswswswswswswsw",
        // "seswwswwwewswewwwwnewwwwne",
        // "esesesesesewneseeseseseseeswseneesesese",
        // "nwnwnenenwneneeneweneneneseseswnenwnenw",
        // "neneneeneswseswwnenenenenenwneneneneenene",
        // "nwnwnwnwnwnenwsenwnwnwnenwnwwnwnenw",
        // "eeeeeeewnweeeseeeeeeee",
        // "wwwnwwnwewnwwnwwnwswnwwnwnewwnw",
        // "swenwswnwswesesenwswswswswnwwnweseesw",
        // "neeneneeneneswneneneswneneneneenenenene",
        // "nwsenwseeeesesese",
        // "wnwwnwnwwnweesewnwswnwswnwnwseswneww",
        // "nwseseswseseeneseneseseseseseswswesesee",
        // "neneeenenwswneeeswneneneenwnenenene",
        // "seswseeswwswswswswswsweswnwse",
        // "neeeneneeneswenenwnenwneeneweswnee",
        // "ewewwswnwneewwnwswsewswwenew",
        // "swwswswswsweswswswswswswswsenewswswsesw",
        // "nwsewnwswnwnwsenwnewwnenwnwnwwwnwewnw",
        // "nwseswwwwwswswswwswneswwswwwwsww",
        // "nenwnwnwnenenenwnenenenenenesenenenene",
        // "eseseswsweseswseswsenesesewwsesesesese",
        // "sesesesesesesesenewnwseseseseseseesese",
        // "nenenenesenewnesenewwenesenenwnenene",
        // "wewewswwswnwwwwwwwswwww",
        // "enwwnwnwnwwwswnewnwnwwnwseewnwsenew",
        // "swseseenwnweswseseswswnenenwnesesewsw",
        // "wwnwwwwsewwwwwwewwwwwww",
        // "nwewnwnwwenwswnwnwenwnwnwnwnwsw",
        // "wsesesenwsenwneeseseewseeswseseese",
        // "enweneeeswnewneswewneneeneeneee",
        // "seswwseswseseseswswesesesesesesesenwsese",
        // "swwnwswwnewsewesesenwwewnew",
        // "nweneeneseneneneenenee",
        // "newnenenenenenwnenesenenenenenene",
        // "wwwwwwwwswwnwswwwsewwwwwne",
        // "swnwswnwenesenwnenew",
        // "nwwwwwwwwwwsewwwwwnewww",
        // "swwsweswnwnenwnwnenwnenewewswsenwswnese",
        // "enenewneswneneneeeeneeseneneneneenee",
        // "nenwnenenenenwnenwwnenenenesesenenwnewne",
        // "swnenenenesweswnenwneswswnwswnwnwneswswne",
        // "wwwwsesewneswwwnewwnewwnewsew",
        // "seswseseseseseeseseswsesesesenenwsesesese",
        // "wwwswwnwnwewwnwwnwwww",
        // "seseseseswsesenwswsese",
        // "swnwswswswswswnwswsweswswswswswswswseswswsw",
        // "nwnenwswnenwnwnwnwsenenwnwnwenwnwnwnwnwne",
        // "seneneneneenwneneswwnwswneneswnesenenw",
        // "swswnewwnenwewswwswswswsesewswsww",
        // "neeswwnwnwnwnwswnewenenwnwnwnwnwesewne",
        // "nwswseswewnewswnwswwsweswseeneseswne",
        // "sesenwswesenweenwwnwesw",
        // "seeseseseswnwseseseswseseswsesesesesese",
        // "esewseeesweseseseseesenenweseese",
        // "nwswseenweeneeeeeseenwseenwsesesw",
        // "enweeeseeesenwsweee",
        // "nenwneneeeneneneeneeseeneeneneneew",
        // "swswseswneswseswswswse",
        // "enenenenwnwnwneswnenwnwnwnwneeneswnenenw",
        // "nenenwneswnenwnwnenwnweswnwnenwnwnenenwne",
        // "swseesewseeenwenwnwsenenwweseese",
        // "swseenenenenenenenenenwwnwnenwwenenene",
        // "nwnwnweneneewwnwsenenenenwswswnesene",
        // "seeswswesenwneeneeenweswesesenww",
        // "eeeeeneseeeeweeeeeesweee",
        // "enwswswneswswswwswswswswseswswswswswswsw",
        // "sewsesesesesesenesesesesesesesesenesesw",
        // "nesenesenenewnweneeenenenenewnee",
        // "swseseseswsweswsesesesweswnwswswseswsesenw",
        // "eeeeeeeweeeeeeweeeeee",
        // "nwnwnwnwnenwnenwnwesenwwnwnwnenwnwnwnw",
        // "wswwnwewnwwwswswwwswswswwswswe",
        // "eeenweeneeeeeeeweeeseseee",
        // "seswseseseswsesesesesenesesesese",
        // "wnwnwwsewswwwewwwnwwneweseswne",
        // "nwnwnwnweswnwnwnwnwwnewnwswnwnwnwnwnw",
        // "seseeseewseswsewsesesewneeseswsenene",
        // "seseneseswseesenwsesesenenwseseswwsesese",
        // "wswwnwenwwnwswnewwwwwnwnenwwnw",
        // "ewewwwwwwswwwwwewnwwsww",
        // "wwsewswwwwnewneswwwswwnenwwne",
        // "wsewnwnwnwwwwwnweeswnewnwwnwnw",
        // "nwnwwnwnwnwnwnwswnwenwewswnwwwwnwnw",
        // "newwwewswsenwnwnwwwsew",
        // "swswwseswnwnweswwsweswwwwswnwswnwse",
        // "eseseseenweewseseseseseeseeeenwe",
        // "eseseeeswesesewnwenwseenwseseesene",
        // "wwnwwnwseewnwnwnenwsesenwnwswnwsene",
        // "eseeseseewseseseeseese",
        // "nwenwneswweneeeswseeseenwneee",
        // "wweswwwswwseswwswswswwwswwnew",
        // "seenwwseswwseeneseenwsenwswe",
        // "nenenenwwneswnenenwnwsesewseneneneenwswnw",
        // "nwnenenenenwneneneswnwnenwnwenene",
        // "sewnwswesewsewneneswwwsenwne",
        // "wnwnwnwnwnenwnenwwswnwnwnwnwnwwwswnw",
        // "sweseneseseswswswswseswnwswseswwswsww",
        // "wenwwwsenwseswnwnwwwnenwnwnwnwnwww",
        // "seneswswswswneswswswswswswswwswswwsw",
        // "nwnwnwenwnwnwwnwnwnwnwnwnwnwnwnwnwswnw",
        // "nwnwesenenenwnenwnwnewnenewnenwnwswnwsenw",
        // "nwwnwwenwnwnwsenwnwnwwewwnwnwwnw",
        // "nwnwnwnwnwnwswnwnwnwnenwnwnenwswnenw",
        // "nwnwnwnenwnwnwnwnwnesenenenenwnwnwnww",
        // "swsenwnwwnesenwnwwnwnwsewnwseenwnwne",
        // "swswenwswseswswswwswswswseswswnwseswswse",
        // "wneneneweseneswswswsenww",
        // "nwneswswnwswnwenenwwneswnwnenenenesenwne",
        // "nenenwneseswnenenenenwneneeneenewee",
        // "nwwwewwnwnwnewsewswnewesewnww",
        // "nwswneswseseseswseseswseseswseseseseswse",
        // "swswwwwwswwwnwswsew",
        // "swsweswneswswswnweswwswswswswswwsww",
        // "swswswswswswswswenwwwswswwswneswswsw",
        // "seeeeseweswswenwneseeseenwnwswseww",
        // "enweeneeenweneeseseneeweseee",
        // "nwswnwenenwnwnwnwswnwnwnwenwsenwwwse",
        // "nweewwnwesenewswnwwswsenwwnwsww",
        // "enwnwswwseswnewsewneeeswseew",
        // "neneenesweeneeneneneneewnwenenese",
        // "swwneneseenesewsesesenewesesesesewsenw",
        // "wnwwnwwwwwewwwwsww",
        // "wnwswwnwsesewwwwwnewnenww",
        // "nwwnwnwnenwswnwneneenwnwswnenwnwnenenw",
        // "eswswewwswwnenwewneneswswewsesw",
        // "neswwswswwswswswwswswswwswwewswww",
        // "eseenwweseseeeeneeeseeenwese",
        // "neeeeweeeeeneenwesesweswneswse",
        // "nwwwnwwwewwnwswnwnwwenwsenwwwse",
        // "nwnwnwnwnwnenenwnwnwnwnwnwnwnenwnesenwsene",
        // "nwseswswswswseswswseswswswswswswswswneswsw",
        // "wnwnenwwenwnwwewwenwwsenwwswwswnw",
        // "seseswneswnwswwswwseseswneenenenesww",
        // "esesewseeneseswwsesesesesesesesesese",
        // "wwnwnwnewnwnwsenwnwwnwenwnwsenwsenw",
        // "swwwneswwwwwwwwwewswwwww",
        // "seneneeseneewswnwswswnewnenwwnenenwswne",
        // "swswseswwwswwwswwswwwwwwwnew",
        // "eeseseewseseseenwseeeeeseseesese",
        // "enenenenwnwswnenwnenwnenenwneswswnenw",
        // "nesewswsesesesesenesesenwseeseneseswswe",
        // "neeneesweswneeswneenwenweenenwsw",
        // "swnenwwsenwneswneneswnenenenenesweene",
        // "wnwewwnwwwwwnwnwsew",
        // "nwnwneswwnwnwswneenwnenwneneeenenwnw",
        // "neeeneseeewneseswnwseswnw",
        // "nwseenenewenwnewneswnenwenwswsenene",
        // "wnenwnewnwnwnwnwnwenwnesenwnwnenenwnwnwnw",
        // "newwwwswwnwwswwswswewsewweww",
        // "neeenenenwneneseneseseenenenwnewnene",
        // "wnweseswseseenwnw",
        // "senwswseswnweseseswseswswswswswswswenw",
        // "nwnwnwnwnwsenwswnwnwnwnenwwnwnwnenesenw",
        // "eneneneenesenesweeeeeeeeeewe",
        // "nwnenwneneswnwnwnwnwnenwnwnenenwewnwnee",
        // "neeenwseeeneeeeneeeeseenenew",
        // "wsweeswseswswswswweswewnwneswseswnw",
        // "swswnwseswswswnwswswswswswswwwswswenwse",
        // "wnweswwswswnwseswswneswswsewswswsw",
        // "nwnenenwnenwnwnenenwewnenwnwnwsenwnwnesw",
        // "wnenenesenwnenenenenesenenenenewnenenenene",
        // "swnwnewswswseseswseswwnwnwsweneswsew",
        // "neeeeneenenenenenwneneswweenenenene",
        // "nwewswswnenenwnwwswweweneeesenwne",
        // "sesewenesesewsewsesenesesenwnesesenwsw",
        // "nwewwnwwwwnwwwwswwwwe",
        // "eseseesewsesesesewneseneeseeseesesew",
        // "neneneeneneeeeeneenenesw",
        // "newewswsesewnenwwwswwwnwwsw",
        // "sewwnwewwsesenenwnwswswnenenw",
        // "nwnwnwsewwswesenee",
        // "ewnwseewswnwwswwwwsw",
        // "wwwenewwesewwwwwwwwwww",
        // "wnwseeswenenwww",
        // "neeseseeesesewwneeeeseswneeseswe",
        // "wwsewswwswwswwswwswnewswswswwsw",
        // "eeseeweseenweseseswsesenesesesese",
        // "senenwwnwwwnwswnwewwnwwswnwwnww",
        // "nwnwnwswnwnenwnwnwsesenenenwnwnenenenenw",
        // "wswnwswswewnwwwnwseneesenewswwew",
        // "nweeswesweeeeeeeeeeeenwe",
        // "eeneeenenwenweeeeseneesweew",
        // "seewswseswsweswwenwswswseswswnweswswsw",
        // "seswenewwswswnewnwwsenenewswswwwse",
        // "nenewweeneeseneeneeswnenenewnene",
        // "neweswnenwnwswneenwwsenwsenwenewnw",
        // "nwnwneseswnenweenenenenesewnenwsenenwwe",
        // "nesweswenesweneenweenewsenenenwnw",
        // "seeeseeeseseseenenweeseeenweseew",
        // "nenewneeswneneneeneneseneeeneneenenee",
        // "swswswswnwwseseseswnwsesenwseseeesesene",
        // "swnwsesenwneseenwsesesesewsesweseeee",
        // "nwneneneneseneneneneneneneew",
        // "wswwwwwwwsenwnwwnwnwwwwneww",
    ]
}
