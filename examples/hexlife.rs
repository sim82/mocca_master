#![no_main]
#![no_std]
#![feature(array_value_iter)]
#![feature(slice_fill)]

use core::convert::Infallible;

use hal::gpio::PullUp;
use mocca_matrix::{bitzet::Bitzet, prelude::*};
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
        let mut data = [RGB8::new(255, 255, 255); NUM_LEDS];
        let mut rainbow = Rainbow::step(1);
        for _ in 0..1 {
            let up = 0..MATRIX_WIDTH;
            let down = (0..MATRIX_WIDTH).rev();
            let pause = core::iter::repeat(20).take(100);
            let pause_short = core::iter::repeat(20).take(20);
            // let pause_short = core::iter::once(8);

            // let mut seq = down.chain(pause_short).chain(up).chain(pause).cycle();
            let mut seq = up.chain(pause_short).chain(down).chain(pause);
            // let mut prev = seq.next().unwrap();
            let mut c = 0;
            const RAMPDOWN: u8 = 8;
            for cur in seq {
                // data.iter_mut().for_each(|v| {
                //     v.r = v.r.saturating_sub(RAMPDOWN);
                //     v.g = v.g.saturating_sub(RAMPDOWN);
                //     v.b = v.b.saturating_sub(RAMPDOWN);
                //     // if v.r < RAMPDOWN {
                //     //     v.r = 0;
                //     // } else {
                //     //     v.r -= RAMPDOWN;
                //     // }
                // });
                data.iter_mut().for_each(|v| {
                    let old = [v.clone(); 1];
                    *v = brightness(old.iter().cloned(), 210).next().unwrap();
                    // if v.r < RAMPDOWN {
                    //     v.r = 0;
                    // } else {
                    //     v.r -= RAMPDOWN;
                    // }
                });
                // let s = seq.next().unwrap();

                // full brightness lags behind one frame (simulate turn on time of 80s lightbulbs)
                // if prev < NUM_LEDS {
                //     for y in 0..MATRIX_HEIGHT {
                //         set_matrix(prev, y, RGB8::new(255, 0, 0), &mut data);
                //     }
                // }
                if cur < MATRIX_WIDTH {
                    let c = rainbow.next().unwrap();

                    for y in 0..MATRIX_HEIGHT {
                        set_matrix(cur, y, c, &mut data);
                    }
                }
                // prev = cur;
                // ws.write(data.iter().cloned()).unwrap();
                ws.write(brightness(data.iter().cloned(), 32)).unwrap();
                // delay.delay_ms(8u8);
                // ws.write(black.iter().cloned()).unwrap();
            }
        }
        delay.delay_ms(200u8);
        ws.write(brightness(data.iter().cloned(), 0)).unwrap();
        // button_wait_debounced(&button, &mut delay);
        run(&mut ws, &mut delay, &button);
        let mut data = [RGB8::new(255, 0, 0); NUM_LEDS];
        ws.write(brightness(data.iter().cloned(), 32)).unwrap();
    }
    unreachable!();
}

fn adjacent(v: Vec2) -> [Vec2; 6] {
    let xshift = v.y().abs() % 2;
    let mut d = [
        Vec2(1, 0),
        Vec2(-1, 0),
        Vec2(-1 + xshift, 1),
        Vec2(0 + xshift, 1),
        Vec2(-1 + xshift, -1),
        Vec2(0 + xshift, -1),
    ];

    d.iter_mut().for_each(|f| *f = *f + v);
    d
}
// #[test]
// fn test_hex() {
//     println!(
//         "{:?}",
//         std::array::IntoIter::new(adjacent(Vec2(0, 0))).collect::<Vec<_>>()
//     );
//     println!("{:?}", adjacent(Vec2(0, 1)));
//     println!("{:?}", adjacent(Vec2(0, -1)));
// }
fn run<WS: SmartLedsWrite<Color = RGB8, Error = hal::spi::Error>>(
    ws: &mut WS,
    delay: &mut Delay,
    button: &dyn InputPin<Error = Infallible>,
) -> Result<(), mocca_matrix::Error> {
    let mut data = [RGB8::default(); NUM_LEDS];
    let mut black = Bitzet::new();
    for (i, line) in input().iter().enumerate() {
        let mut c = line.chars();
        let mut x = 0i32;
        let mut y = 0i32;

        // data[i % NUM_LEDS] = RGB8::new(0, 255, 0);

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
        }
        if x.abs() > 3 || y.abs() > 3 {
            continue;
        }
        if black.contains(&Vec2(x, y)) {
            black.remove(&Vec2(x, y));
        } else {
            black.insert(Vec2(x, y));
        }
        set_matrix(
            x as usize + 10,
            y as usize + 10,
            RGB8::new(0, 0, 255),
            &mut data,
        );
        // if i % 10 == 0 {
        ws.write(brightness(data.iter().cloned(), 32)).unwrap();
        // if i == 100 {
        //     break;
        // }
        // }
        // delay.delay_ms(200u8);
        // println!("{} {}", x, y);
    }
    // println!("len: {}", black.len());
    for v in black.iter() {
        set_matrix(
            (v.x() + 10) as usize,
            (v.y() + 10) as usize,
            RGB8::new(64, 64, 255),
            &mut data,
        );
    }

    let mut i: usize = 0;
    let mut turn_off = [true; NUM_LEDS];
    loop {
        let warp_mode = button.is_low().unwrap();
        if i % 25 == 0 || warp_mode {
            let mut rainbow = Rainbow::step(7);

            if true {
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
                    .collect::<Bitzet>();

                let white = white.difference(&black);
                for v in white.iter() {
                    let n = adjacent(v).iter().filter(|v| black.contains(*v)).count();
                    if n == 2 && v.x().abs() < 15 && v.y().abs() < 15 {
                        black_new.insert(v);
                    }
                }

                black = black_new;
            } else {
                for q in 0..4 {
                    let j = i % (128);
                    //for j in 0..(i % 128) {
                    black.quadrants[q].bit_set(j);
                    black.max[q] = j;
                    // }
                }
            }

            turn_off.fill(true);
            if warp_mode {
                data.fill(RGB8::default());
            }
            for v in black.iter() {
                if let Ok(addr) = set_matrix(
                    (v.x() + 10) as usize,
                    (v.y() + 10) as usize,
                    rainbow.next().unwrap(),
                    &mut data,
                ) {
                    turn_off[addr as usize] = false;
                }
            }
        }
        i = i.overflowing_add(1).0;

        if button.is_high().unwrap() {
            for (i, b) in turn_off.iter().enumerate() {
                if *b {
                    let v = &mut data[i];
                    let old = [v.clone(); 1];
                    *v = brightness(old.iter().cloned(), 220).next().unwrap();
                }
            }
        }
        // data.iter_mut().for_each(|v| {
        //     let old = [v.clone(); 1];
        //     *v = brightness(old.iter().cloned(), 210).next().unwrap();
        //     // if v.r < RAMPDOWN {
        //     //     v.r = 0;
        //     // } else {
        //     //     v.r -= RAMPDOWN;
        //     // }
        // });
        // ws.write(data.iter().cloned()).unwrap();
        ws.write(brightness(data.iter().cloned(), 16)).unwrap();
        // println!("day: {} {}", i, black.len());
    }
    Ok(())
}

fn input() -> &'static [&'static str] {
    &[
        "swswswswswnwswswswsweswsesw",
        "wneswseseneswnweneswwswseswwnwseswswe",
        "ewswswswwswnwnwsweswwwwwswwwswwsw",
        "senwseseswseseswswesewseseseseseswsese",
        "seswnwseseswseseswswseseswswseswesese",
        "swnwnenwnenwnwnwswnwnwnwnwnwnwnwenwnwnw",
        "seseeseneswnwseseneeeseeswsewseseese",
        "senenwnwnewnwnwnwnenwnenwnwnwnenwswnenw",
        "eneneneweeneneeneneneeneneenesew",
        "nenesenewwenwseseseseswsesewneseweswse",
        "swswweswswswswswswswseswnenwswswsweswsw",
        "nwswnwnwnwnwnwnenenenwnwnw",
        "seseeneseeesenesewseswsewseeeese",
        "wwseswwnwneswsewswwswswswwswswswswne",
        "wwewwnwwwswsenwnewnwwnwsewwww",
        "wnwnwnwenwnwnwnenwnweswnwnwwswnwnwnw",
        "neeneneswnenenenenwnenenwnenenenenewnwe",
        "nenwsenwnwnwnwnwswswnwnwnwenwnwnwnwnwnw",
        "swenwsenweenwswnwswseseeswswesenwne",
        "ewseswsewswneewneneneeneseneeswnwsw",
        "nwswnwswewswswswesesweswswse",
        "swswswswenwswswswwswneswneswswswsesww",
        "seswswseseswswsenwswse",
        "wwwwwwwswsewnewwnwwwwwew",
        "swwwswewswswswswswnwswswswswseswsww",
        "sesewsewneseneseseseswswseeseseswnesw",
        "wwswswwwwswwewwswwwsewnwnww",
        "nwnenwnenenwnenwnwnwsenwnwne",
        "neewwwneswseseeeneeseesesesewee",
        "senwneeneneenewneneneneneneneswnenenene",
        "swswswneswwswswwswswsewneswesw",
        "nenenenenwnenesewnenenenenenenwnwnenwne",
        "nwnwnwsesenwnwwsewnwnenwnewnwnwwwnwnw",
        "nwnenwwnweswseseeswenenwweneneeswswsw",
        "eeesweeeenweseseweeneeseeese",
        "swswseseswseswnwseseswswswseseseneswswsese",
        "swnwnwewnewwwwnwnwwnwwswnwse",
        "sesenwnenenwnenwnwnwnenwnewnwnwnw",
        "neswneneeneneenenenenewnwneneswnenwewne",
        "nwnenwswnwenwnwnwnwnwnwnwswnwnwnwnwnwnwnw",
        "nwwwewwneewswneswnwwwwwsewsew",
        "wswswswswswweswwwwswseswswswneswswe",
        "wnwwnwnwnewnwnwnwwnwwsenwwnwnwwnw",
        "wswwwswswwwwnesew",
        "nwnwneenenwnwswnwnwnwswnwsenwnwwenenenwnw",
        "swsewseseswswseswneseseswseseswseseseese",
        "swseswseswnwsewneweseseseeswsesesesew",
        "sweswwwneswswwswesenenwsweswswswswse",
        "wswseneeseweeeeweseesenwnwseew",
        "wwnwnwwwnwnwnwnwwnwnwnwnwswwnwwe",
        "swnwnwwseneneneneneneswswswnewenwsese",
        "seseseseseesesenwseseneesewswseswnwnw",
        "neswswsenwesenesenesenesesewwneswnwswnwne",
        "nesesesenenwnwnwnenwnwswnwneenewnenwne",
        "neneneneneeneenewseewnenesenenenenene",
        "eseseseseneseeseseesenwseseseseseseesww",
        "neeenenesweeeeeeweneeeeeene",
        "enwwwwwsewwwnwnwwnwwwwwnwnwnw",
        "neseeeeseseseeseeseneeseseseenwwsw",
        "enenesewneenenenwseseneeweeswenwene",
        "wneneseneneneenenenenenwnenenenenenene",
        "wnwneeswewnwnwseswswsweswsenewswsw",
        "eseeswsenwenweseseeseseseesenesesese",
        "wswswseseseseneenesesesewsesenewsew",
        "wnwwwwwwewwswwwwwwwswsww",
        "swenewwwseswwnesesenewneswseneswswswsw",
        "eswseseseseesewseseneseenwsewseee",
        "swneneswneneneneeneeenwneeenenwwenene",
        "neswnenesenenwnenenenenenenenenenenenwnene",
        "sesesesesewsesesweseseseseseseesesenee",
        "nwnwnwnwnwnwswnwnesewnwnwnwwnwnwnwnenw",
        "eenesewwwwnwnwwwwnwwwsw",
        "swnenwwswswswneseeseneswneseswwseswsw",
        "neneenenwnenenwnwnwseswnwnwnenwnenenenenw",
        "neenenwneneseneeneneeneneeeeneswnee",
        "swswswswneseswwswswseswswweneenwnew",
        "neseewsesesewswsese",
        "enenweneeneneeeeneeswswneneeeneenw",
        "seeeeneeeenwseewswenewnwseesew",
        "sweeeneesenweswewsweseeseenenwee",
        "seesesewswenenwnweswsewseneenwsesw",
        "swwswwswwwneswwsww",
        "wwwwwwwwwwwwewwwweww",
        "neneneseneeneneeswneenesenwnewnenwe",
        "nwnwnwnwnwnenwnwnwnwsenwnwnwwnwnwnwnwnw",
        "eeeeeseseeenweeeesweeenee",
        "nwnenwnwneneseneswnenwnenwwnwnwnenenwnwnw",
        "sewswswwsewwneesewnewnewne",
        "nwnwwnweswnwneswswnwnwnwnenwnweswenwe",
        "nwneenenenenwneneneeneseeneneswnenenene",
        "swwwwswswswswnewwwwwsw",
        "neneneneeneeneeneneeewnwneswneneesene",
        "nenenewnenenenenenenewnenwnwneesenene",
        "swnwswswwwswswwweswnewswswwswnwswsee",
        "neseseeseseeseseseeeseseewesewse",
        "nwnenenesesenwnwswnwwswwnwnwnenwnwsew",
        "seesesewseneseseseseseseswsesesenwswsese",
        "nwsenwwnenwneseswwwnwwnewesewwswnw",
        "sesesesenesesesesesewsewsesesesesesesesee",
        "seswwnwseeswnwwneeneseweeseenee",
        "nwseenwnwnwnewswnw",
        "nwnwnenwsenwnwnwnwnww",
        "wnwwwnwenwnwwsenewnwnwnwwsewwnenw",
        "sesenwsenwseswseseseseeewsenesesesesese",
        "seeneeeeseeenwnwsweeeeneeene",
        "eeeeseeeeswneeneeneesweenew",
        "swseneswswnwseswswseswswswswneswswswsesw",
        "nesesweneseseneeweesenwnwseeswnwswse",
        "senwswnenenwnwneeesw",
        "nweeseeenweesweeeeeseeswese",
        "sweneenwneeneeneesenwnwnwnesweswswnee",
        "swswnwewswnwswwwwswwswwwewswswswe",
        "swswswsenwswwswsweswswnwsweswswesesw",
        "swswsesesenenwewseseswseeswseseswsesesw",
        "seswnewewswnenwenwnenenenenenenwwene",
        "wnwswswwwsewnwnwswweseswsenewwww",
        "wswswweswwwnwsewwswswwswsw",
        "swswseswsweswswswswswswswswswswnwswswsw",
        "nenwnewnenenenwnenenwnwnenwneneneenese",
        "seswswswswseneswsesw",
        "neseneneenewsenwwnwnwwswwnwwee",
        "swswswwswswswwnwnewwwneeneswwnwse",
        "nwswwwwnwswesewsenwseswwswwwwwnew",
        "wsenweeeseeeseeseeeeeeeseese",
        "seseseseseseeswseesesesenesesewnesese",
        "nenesesewsenwneseseseneseswseswwswsesese",
        "sewseseewsenenweewswseseneeswsee",
        "swseewnwsenwnenwswneswnwwswnesenwnewnw",
        "neeeewseseneswnenwneneweneneseseew",
        "nenwnenwswneneenenwnesesenewneneswnenwnw",
        "nenewnenesweneneenesenenenenenenenwneee",
        "wnwsenwnwsenenenesesesenesewnwnwnwnwnenw",
        "nwswswswnewnwsweseswnesenweswenwesww",
        "nwnwnenesenwnwwnwneswnenwnwnwnwnwenwnw",
        "seswseseswswneswnwswseswsesesenwswswswswsw",
        "nenwnwsenwnwnwnenwnenenenwsenwwnwnwnenwnwne",
        "eswwswswwswswswswswswwsww",
        "nenesenwenwsweesweeneenw",
        "nenenesenenenenwnwnenenewnenwswnwnenenwne",
        "eeseeeneseewneeeeseeweeeese",
        "swswwseseswsenwseeswswswsesesw",
        "nwnwsenwsenenenwswseswsewseswswwnwswnew",
        "nwwnwnwnwenwnwwwnwwnwwwnwnwwwsw",
        "wewnwwswswnwwnwnwnwnwewswnwesenwwne",
        "nenenwnwnwnwenewenwsenwnenenwnwnewnwne",
        "seseseenwseseseseseswwseswnwsee",
        "swseswseseseswsesesesenwseseswsese",
        "swswneseswseswwswswswseswsweswswswswswsw",
        "seneneeseseswseseswsenwseeseseswsesee",
        "nwneneseneneneneneenesenewseneewnene",
        "nwwnenwnenenwnwsenwnwsenwnwnwnwnenenenw",
        "wwnwwsewwwwwsewwwnwnwnwnwwnese",
        "seseswnwseswseswsesesenwsesesese",
        "wnweswsenwnwswnenwnwnwswwsewnenwnwwnw",
        "wseswnwseswswseswswsenwswswsesene",
        "neeeneenwneneeeeneeesweenenweesw",
        "wwwnwnwwwwwwwwwnwwwwesew",
        "neseeneneneneneneneneneneenwnenwneneswnene",
        "neeeesenweeswenewne",
        "nwswnwenwwwwnwswwwwwwnwnwenwnwe",
        "wwwwswwwwwwswwwwwewsw",
        "eeseeswswswneeeweewneneneenenene",
        "swswswesesesenwswseswnese",
        "sesewswseseseswseseseseseseseseseswnese",
        "neeeeeneeweeeesweeenenwe",
        "nwwnesenwneneswenenenenenwnwnenwnwwnene",
        "neeenwneneswneneneswneeeneswneene",
        "nwwewwnwnwnwswwswwwnwwwwwwnew",
        "nenwnwnwwswnwwnwnenwesee",
        "wswswswswwseswswswswnewswswnwswwswsww",
        "neneneneswsenenenwne",
        "neeeweeeeeeeseneneneneeesewe",
        "nwswnwnwnwnwenwneeswnwnwwnwnw",
        "swsesenwseswseseswwseneseneseseesesesese",
        "nweeweeeeeseeseeswseeee",
        "nwsesesesesewesenwnenwswneeseneswsesw",
        "nwneneenenewsweeneneneneneneeewnene",
        "swseeseneswwneswnwwswneewnesw",
        "neseseswswseswseswseswsenwsweseswsesesese",
        "seswswwswwwwewswswwnewwwswswnww",
        "senwseesesesesesesesesesesesenwsenwsese",
        "seswnwsesesewseseneseseseesenwsesesesewse",
        "neeenwnesweenwneeeeeeeeeneswene",
        "seenwsenweswnwswwnwnwnwnwnewnwswenw",
        "swswswswswswswswswswwswswseswne",
        "enwswswnwswneswsewnwswseneseenwwnwsw",
        "swneswnenenwnwneeeneneneeswneswnenesew",
        "nwswnwwswwseewseewsenenwnwwwnwwwse",
        "eeeseesenesesewseeseswseneesesesese",
        "swnewswseswwwnwwewew",
        "eeeeeeseeeeeswnwe",
        "nenenenenwnwneeneswwneeneneneswnenenesw",
        "eswwswswswsweswnw",
        "newnwsenwnwnenwnwenwnenenwnwneswnwwswnene",
        "eeneeneneeeneeweeswneeneseeewne",
        "nwwnenewenenwnwnwnwe",
        "neneswwneswneneneswene",
        "swwneneweneeswwneenesenwnwenenese",
        "seesewsesesesesenesesesesewsesesesesese",
        "nesenwesesesenwwswsesewesesesesesesesese",
        "nwsenwneeweenweswnwnwnwwwenwwne",
        "seswswswnwswseswswswseswenwswswswswnesw",
        "swnwsesweswswseswwswswwseeswswsenesesw",
        "ewseseneeseeeneeenwnweeesenwne",
        "neswswswswswswswswseseswwenwswnwswswsw",
        "nwswswwseeswswswswswswswswswswswswswne",
        "nenwnenenenenwnenenenewnenenwneenwnw",
        "senweeeenwseeseeenwseeseseeeese",
        "seseseswseseswswseseseneneseseswswwseswswse",
        "swswwswwswwswswswswwwwswwswwe",
        "sesesewseswseseseseseseseesesesese",
        "nwwnwnwnwnwenwwnwnwnwwnwnw",
        "nwnwnwnwnewnenwnenwnenwnesenenwnenenwnw",
        "nwwseneesenwwswwneseenwseswwwnwe",
        "enwnwswsesenwswsesweeseseseseewnwswse",
        "eeeeseseeesweneeeewewwse",
        "ewneneeseeneseswswnwswwseswesesene",
        "eeeneeeneeweeneeeeeeweese",
        "nwnesenwnwnewnwnenwnenwnwneeswnwnwneswe",
        "swseweeeseseneeeeseneseseeneeew",
        "seewnenenwenesesewnwnwnwseswe",
        "nesenewsesweseseswwsesewneswswswese",
        "neenenesenwneeneeneswswneneenenenene",
        "wneswsenenwsewseeswseswswnesewswnese",
        "senwnwnenenewwwnenwenwnenwseenenenw",
        "nwwwsewneswswwnenwswnewnenwseesenenwnw",
        "eneneneeneseswweeneeewnweneee",
        "seenenwswswswnwnwwseneneseeeeeswsw",
        "nenenesenewnenenwneneneneneneenenenenene",
        "wswwwwwwswwswwnwwswwswewwew",
        "neenweneneseeneswwswnew",
        "eeeenweeseneeneeeeeneeswee",
        "nwsesewsesesewnwseswneseseesesesenwse",
        "wnwnenenenenwwesweeneneswsenwnwswsene",
        "swswwswswswswswswwsweneswnwwnwswesw",
        "wwewwwwnwww",
        "eeeeneswsweneenwnenwneneneeneene",
        "sewseswswswswswseseswseewnwnenwsw",
        "neeneseweeneeneesweeneenweese",
        "sweeeeeeneeeeeeee",
        "wnwnwwnenewwnwnwwsewswwwwnwnwwnww",
        "seswwseseseswnwswseeswswswnwseswseswswse",
        "eseswnenwweeswnwwneeswnese",
        "sesesewnewwwwneneww",
        "nweneesweneesenewnenw",
        "eesesesenwseseeseseseseseese",
        "neswswnewenenenenenenwenenenwne",
        "swnenwnwnwsenwwnwwwneewwnwwwnenwwse",
        "seswswsesewweneenenwsesewswsw",
        "seeseeeswenwswnweeeswnweeeeee",
        "sweswneswwwwswne",
        "seseswseswswseneswswseswseswneswswseswsese",
        "wnwwnwnwnwwwwwnwwwwnwenww",
        "wswseswswswswsweseswswnwseswneswswswsw",
        "enwwnwnwwseeesewnww",
        "nwnwsweseswswswwswswseswswweswnwswe",
        "swseswswswwwswswnewsw",
        "wswnewnewnewenwsewwwseseswnwnwwse",
        "eseneseseeesesweesesewseseesesewse",
        "neewseeseeeswswsesesenenw",
        "newneweenenesewseewwnwwsenesesee",
        "wnenwwswwswswnwnweeenwneswseenwnwnw",
        "swnwnwnwnwenwnenwnwnwnwnwnw",
        "neneesewnweseeneeneseewsewnwswne",
        "wwwwwenwwwwwwswwwwwnwsew",
        "seesweneseeseweeseneseeseseeee",
        "nwnwnwsewwnwwnwnwnwnwnwnwnwnwnwneswwse",
        "newnwswnwwnwnwnwnwwwnwnwnwwwnwnwenw",
        "seweeeseeeseeseeeeeseswnweseene",
        "eseseeeeeswesesweeseneeesenesee",
        "eseswwsesenwseseswseseseeswsenese",
        "wnwnwnwnwnenwnwenenw",
        "neneenesweswsenwseneneswnenwwwneswneww",
        "wswneswewswseswswswswwsw",
        "seseeneeseswsewneswnwseneswswneenwnee",
        "nwwnesenwsenwnenwnwnesewnwswswnwneswwnw",
        "enweneeeneeneneeneneswseneseenwne",
        "nwnenwnenenwnwnwsenwnwnewwnwnenwene",
        "wwnwneswwewnwweswwneseswwswnesew",
        "enewneewsewenenenenesenenewene",
        "nwsenwnwnwnenenenenenenenwnwnwnenw",
        "nenenenenenwnwneneneswne",
        "enwneseeneneneneswwneeneneswe",
        "swswwswsewsewwwwneeeswwwswnwww",
        "swnwseenenenwenwnenwnwwnwswneswnwnwnenw",
        "wswswwswwwswswswswswswwswweswnwsw",
        "seseswwneswwswswswswswswneswswswswswsww",
        "seseswnwnwnwnwnwnwnwnwwewnwnw",
        "wwnwnwwwnwnewnwsewwnwnwnwnwnwewsw",
        "wseswnwwneswnenewwseewseswswwwne",
        "wswwwwwwwwwnwewwwnw",
        "senwswseneseseseswswswsweswswseswseswsw",
        "wwwswwwswwwsewnwswwnewwswswswsw",
        "swseswswwswseswswseswseseswseswne",
        "ewswswsweswswnewswseneseswseswsenwnwe",
        "wwwwwsenwwwwwwwwwwne",
        "nwsewwwwsewnwwwnwne",
        "swswswswswnewswwwwwswswswswswsw",
        "nwwseenwsenenwwnwnwnwnwenwwnwnwwsw",
        "eswseswswsenenenenwswseswswswseswsenwswse",
        "wswwswwswwsewwwswswswwnwwnene",
        "sesesesesesenewwswwseneseseeswsesesese",
        "swnwnwnwnwnwnwnwnenwnwnwenenwnwnwwnwnwnw",
        "wneenenwneenwwnenwnwnwneneneneeneswnw",
        "swwwwwwwwwwwwwwwnewseww",
        "senwneswseswsesenwsesee",
        "neswwseseeswseseswswsesww",
        "nwswseswseseswseneeseseneseseseswseswnesw",
        "eneeeneneneewenenenenenweenesene",
        "wnwwnenenwnenwswnwnenweenwnwnenese",
        "wswseswswneswswswnweswswsweseseswsene",
        "senwwseneseeesewnesewswsewsesenwnese",
        "esenesesesesesesesesesesewsesesewsesw",
        "neeneneseswnwnesenwwnwnwswnenwne",
        "neenenwnenewneswnewneneneenenenenenwe",
        "swseswswswswseswswnwswsweswsesesesesw",
        "nwenwsewnwnenenwnwwnwnwnwswnwnwnwnwwnw",
        "wsenwwnwswneweswwweewnewesene",
        "swwnwwswwwnwnwewewewwwnwnwnww",
        "wsewwnwswsweswswswswswswswswswswswswsw",
        "seswwswwwewswewwwwnewwwwne",
        "esesesesesewneseeseseseseeswseneesesese",
        "nwnwnenenwneneeneweneneneseseswnenwnenw",
        "neneneeneswseswwnenenenenenwneneneneenene",
        "nwnwnwnwnwnenwsenwnwnwnenwnwwnwnenw",
        "eeeeeeewnweeeseeeeeeee",
        "wwwnwwnwewnwwnwwnwswnwwnwnewwnw",
        "swenwswnwswesesenwswswswswnwwnweseesw",
        "neeneneeneneswneneneswneneneneenenenene",
        "nwsenwseeeesesese",
        "wnwwnwnwwnweesewnwswnwswnwnwseswneww",
        "nwseseswseseeneseneseseseseseswswesesee",
        "neneeenenwswneeeswneneneenwnenenene",
        "seswseeswwswswswswswsweswnwse",
        "neeeneneeneswenenwnenwneeneweswnee",
        "ewewwswnwneewwnwswsewswwenew",
        "swwswswswsweswswswswswswswsenewswswsesw",
        "nwsewnwswnwnwsenwnewwnenwnwnwwwnwewnw",
        "nwseswwwwwswswswwswneswwswwwwsww",
        "nenwnwnwnenenenwnenenenenenesenenenene",
        "eseseswsweseswseswsenesesewwsesesesese",
        "sesesesesesesesenewnwseseseseseseesese",
        "nenenenesenewnesenewwenesenenwnenene",
        "wewewswwswnwwwwwwwswwww",
        "enwwnwnwnwwwswnewnwnwwnwseewnwsenew",
        "swseseenwnweswseseswswnenenwnesesewsw",
        "wwnwwwwsewwwwwwewwwwwww",
        "nwewnwnwwenwswnwnwenwnwnwnwnwsw",
        "wsesesenwsenwneeseseewseeswseseese",
        "enweneeeswnewneswewneneeneeneee",
        "seswwseswseseseswswesesesesesesesenwsese",
        "swwnwswwnewsewesesenwwewnew",
        "nweneeneseneneneenenee",
        "newnenenenenenwnenesenenenenenene",
        "wwwwwwwwswwnwswwwsewwwwwne",
        "swnwswnwenesenwnenew",
        "nwwwwwwwwwwsewwwwwnewww",
        "swwsweswnwnenwnwnenwnenewewswsenwswnese",
        "enenewneswneneneeeeneeseneneneneenee",
        "nenwnenenenenwnenwwnenenenesesenenwnewne",
        "swnenenenesweswnenwneswswnwswnwnwneswswne",
        "wwwwsesewneswwwnewwnewwnewsew",
        "seswseseseseseeseseswsesesesenenwsesesese",
        "wwwswwnwnwewwnwwnwwww",
        "seseseseswsesenwswsese",
        "swnwswswswswswnwswsweswswswswswswswseswswsw",
        "nwnenwswnenwnwnwnwsenenwnwnwenwnwnwnwnwne",
        "seneneneneenwneneswwnwswneneswnesenenw",
        "swswnewwnenwewswwswswswsesewswsww",
        "neeswwnwnwnwnwswnewenenwnwnwnwnwesewne",
        "nwswseswewnewswnwswwsweswseeneseswne",
        "sesenwswesenweenwwnwesw",
        "seeseseseswnwseseseswseseswsesesesesese",
        "esewseeesweseseseseesenenweseese",
        "nwswseenweeneeeeeseenwseenwsesesw",
        "enweeeseeesenwsweee",
        "nenwneneeeneneneeneeseeneeneneneew",
        "swswseswneswseswswswse",
        "enenenenwnwnwneswnenwnwnwnwneeneswnenenw",
        "nenenwneswnenwnwnenwnweswnwnenwnwnenenwne",
        "swseesewseeenwenwnwsenenwweseese",
        "swseenenenenenenenenenwwnwnenwwenenene",
        "nwnwnweneneewwnwsenenenenwswswnesene",
        "seeswswesenwneeneeenweswesesenww",
        "eeeeeneseeeeweeeeeesweee",
        "enwswswneswswswwswswswswseswswswswswswsw",
        "sewsesesesesesenesesesesesesesesenesesw",
        "nesenesenenewnweneeenenenenewnee",
        "swseseseswsweswsesesesweswnwswswseswsesenw",
        "eeeeeeeweeeeeeweeeeee",
        "nwnwnwnwnenwnenwnwesenwwnwnwnenwnwnwnw",
        "wswwnwewnwwwswswwwswswswwswswe",
        "eeenweeneeeeeeeweeeseseee",
        "seswseseseswsesesesesenesesesese",
        "wnwnwwsewswwwewwwnwwneweseswne",
        "nwnwnwnweswnwnwnwnwwnewnwswnwnwnwnwnw",
        "seseeseewseswsewsesesewneeseswsenene",
        "seseneseswseesenwsesesenenwseseswwsesese",
        "wswwnwenwwnwswnewwwwwnwnenwwnw",
        "ewewwwwwwswwwwwewnwwsww",
        "wwsewswwwwnewneswwwswwnenwwne",
        "wsewnwnwnwwwwwnweeswnewnwwnwnw",
        "nwnwwnwnwnwnwnwswnwenwewswnwwwwnwnw",
        "newwwewswsenwnwnwwwsew",
        "swswwseswnwnweswwsweswwwwswnwswnwse",
        "eseseseenweewseseseseseeseeeenwe",
        "eseseeeswesesewnwenwseenwseseesene",
        "wwnwwnwseewnwnwnenwsesenwnwswnwsene",
        "eseeseseewseseseeseese",
        "nwenwneswweneeeswseeseenwneee",
        "wweswwwswwseswwswswswwwswwnew",
        "seenwwseswwseeneseenwsenwswe",
        "nenenenwwneswnenenwnwsesewseneneneenwswnw",
        "nwnenenenenwneneneswnwnenwnwenene",
        "sewnwswesewsewneneswwwsenwne",
        "wnwnwnwnwnenwnenwwswnwnwnwnwnwwwswnw",
        "sweseneseseswswswswseswnwswseswwswsww",
        "wenwwwsenwseswnwnwwwnenwnwnwnwnwww",
        "seneswswswswneswswswswswswswwswswwsw",
        "nwnwnwenwnwnwwnwnwnwnwnwnwnwnwnwnwswnw",
        "nwnwesenenenwnenwnwnewnenewnenwnwswnwsenw",
        "nwwnwwenwnwnwsenwnwnwwewwnwnwwnw",
        "nwnwnwnwnwnwswnwnwnwnenwnwnenwswnenw",
        "nwnwnwnenwnwnwnwnwnesenenenenwnwnwnww",
        "swsenwnwwnesenwnwwnwnwsewnwseenwnwne",
        "swswenwswseswswswwswswswseswswnwseswswse",
        "wneneneweseneswswswsenww",
        "nwneswswnwswnwenenwwneswnwnenenenesenwne",
        "nenenwneseswnenenenenwneneeneenewee",
        "nwwwewwnwnwnewsewswnewesewnww",
        "nwswneswseseseswseseswseseswseseseseswse",
        "swswwwwwswwwnwswsew",
        "swsweswneswswswnweswwswswswswswwsww",
        "swswswswswswswswenwwwswswwswneswswsw",
        "seeeeseweswswenwneseeseenwnwswseww",
        "enweeneeenweneeseseneeweseee",
        "nwswnwenenwnwnwnwswnwnwnwenwsenwwwse",
        "nweewwnwesenewswnwwswsenwwnwsww",
        "enwnwswwseswnewsewneeeswseew",
        "neneenesweeneeneneneneewnwenenese",
        "swwneneseenesewsesesenewesesesesewsenw",
        "wnwwnwwwwwewwwwsww",
        "wnwswwnwsesewwwwwnewnenww",
        "nwwnwnwnenwswnwneneenwnwswnenwnwnenenw",
        "eswswewwswwnenwewneneswswewsesw",
        "neswwswswwswswswwswswswwswwewswww",
        "eseenwweseseeeeneeeseeenwese",
        "neeeeweeeeeneenwesesweswneswse",
        "nwwwnwwwewwnwswnwnwwenwsenwwwse",
        "nwnwnwnwnwnenenwnwnwnwnwnwnwnenwnesenwsene",
        "nwseswswswswseswswseswswswswswswswswneswsw",
        "wnwnenwwenwnwwewwenwwsenwwswwswnw",
        "seseswneswnwswwswwseseswneenenenesww",
        "esesewseeneseswwsesesesesesesesesese",
        "wwnwnwnewnwnwsenwnwwnwenwnwsenwsenw",
        "swwwneswwwwwwwwwewswwwww",
        "seneneeseneewswnwswswnewnenwwnenenwswne",
        "swswseswwwswwwswwswwwwwwwnew",
        "eeseseewseseseenwseeeeeseseesese",
        "enenenenwnwswnenwnenwnenenwneswswnenw",
        "nesewswsesesesesenesesenwseeseneseswswe",
        "neeneesweswneeswneenwenweenenwsw",
        "swnenwwsenwneswneneswnenenenenesweene",
        "wnwewwnwwwwwnwnwsew",
        "nwnwneswwnwnwswneenwnenwneneeenenwnw",
        "neeeneseeewneseswnwseswnw",
        "nwseenenewenwnewneswnenwenwswsenene",
        "wnenwnewnwnwnwnwnwenwnesenwnwnenenwnwnwnw",
        "newwwwswwnwwswwswswewsewweww",
        "neeenenenwneneseneseseenenenwnewnene",
        "wnweseswseseenwnw",
        "senwswseswnweseseswseswswswswswswswenw",
        "nwnwnwnwnwsenwswnwnwnwnenwwnwnwnenesenw",
        "eneneneenesenesweeeeeeeeeewe",
        "nwnenwneneswnwnwnwnwnenwnwnenenwewnwnee",
        "neeenwseeeneeeeneeeeseenenew",
        "wsweeswseswswswswweswewnwneswseswnw",
        "swswnwseswswswnwswswswswswswwwswswenwse",
        "wnweswwswswnwseswswneswswsewswswsw",
        "nwnenenwnenwnwnenenwewnenwnwnwsenwnwnesw",
        "wnenenesenwnenenenenesenenenenewnenenenene",
        "swnwnewswswseseswseswwnwnwsweneswsew",
        "neeeeneenenenenenwneneswweenenenene",
        "nwewswswnenenwnwwswweweneeesenwne",
        "sesewenesesewsewsesenesesenwnesesenwsw",
        "nwewwnwwwwnwwwwswwwwe",
        "eseseesewsesesesewneseneeseeseesesew",
        "neneneeneneeeeeneenenesw",
        "newewswsesewnenwwwswwwnwwsw",
        "sewwnwewwsesenenwnwswswnenenw",
        "nwnwnwsewwswesenee",
        "ewnwseewswnwwswwwwsw",
        "wwwenewwesewwwwwwwwwww",
        "wnwseeswenenwww",
        "neeseseeesesewwneeeeseswneeseswe",
        "wwsewswwswwswwswwswnewswswswwsw",
        "eeseeweseenweseseswsesenesesesese",
        "senenwwnwwwnwswnwewwnwwswnwwnww",
        "nwnwnwswnwnenwnwnwsesenenenwnwnenenenenw",
        "wswnwswswewnwwwnwseneesenewswwew",
        "nweeswesweeeeeeeeeeeenwe",
        "eeneeenenwenweeeeseneesweew",
        "seewswseswsweswwenwswswseswswnweswswsw",
        "seswenewwswswnewnwwsenenewswswwwse",
        "nenewweeneeseneeneeswnenenewnene",
        "neweswnenwnwswneenwwsenwsenwenewnw",
        "nwnwneseswnenweenenenenesewnenwsenenwwe",
        "nesweswenesweneenweenewsenenenwnw",
        "seeeseeeseseseenenweeseeenweseew",
        "nenewneeswneneneeneneseneeeneneenenee",
        "swswswswnwwseseseswnwsesenwseseeesesene",
        "swnwsesenwneseenwsesesesewsesweseeee",
        "nwneneneneseneneneneneneneew",
        "wswwwwwwwsenwnwwnwnwwwwneww",
    ]
}
