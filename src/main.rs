#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use alloc::boxed::Box;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    gpio::{AnyPin, Input, Io, Level, Output, Pull},
    mcpwm::{
        operator::{PwmPin, PwmPinConfig},
        timer::PwmWorkingMode,
        McPwm, PeripheralClockConfig,
    },
    peripherals::MCPWM0,
    prelude::*,
    timer::timg::TimerGroup,
};
use filament_changer::FilamentChanger;
use serde::Deserialize;

mod filament_changer;

extern crate alloc;

#[embassy_executor::task]
async fn filament_changer_task(mut filament_changer: FilamentChanger<'static>) {
    filament_changer.run().await;
}

/*
Servo Motor Limits:
    300 is min
    2500 is max
    0deg is 500
    90deg is 1500
    180deg is 2500
*/

#[derive(Deserialize, Clone, Debug)]
struct MoveCommand {
    steps: i32,
    stepper: i8,
}

impl Default for MoveCommand {
    fn default() -> Self {
        MoveCommand {
            steps: 500,
            stepper: 0,
        }
    }
}

const QUEUE_SIZE: usize = 10;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    esp_println::println!("Init!");

    let peripherals = esp_hal::init(esp_hal::Config::default());

    esp_alloc::heap_allocator!(72 * 1024);

    // initialize embasy
    let timg1 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timg1.timer0);

    // initialize filament changer
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);

    let clock_cfg = PeripheralClockConfig::with_frequency(32.MHz()).unwrap();

    let mut mcpwm = McPwm::new(peripherals.MCPWM0, clock_cfg);
    let servo_pin = Output::new(io.pins.gpio23, Level::Low);

    // connect operator0 to timer0
    mcpwm.operator0.set_timer(&mcpwm.timer0);
    // connect operator0 to pin
    let mut pwm_pin: PwmPin<'_, AnyPin, MCPWM0, 0, true> = mcpwm
        .operator0
        .with_pin_a::<AnyPin>(servo_pin, PwmPinConfig::UP_ACTIVE_HIGH);

    // start timer with timestamp values in the range of 0..=99 and a frequency of
    // 20 kHz
    let timer_clock_cfg = clock_cfg
        .timer_clock_with_frequency(20000, PwmWorkingMode::Increase, 50.Hz())
        .unwrap();
    mcpwm.timer0.start(timer_clock_cfg);

    pwm_pin.set_timestamp(1000);

    let stepper_a_dir = Output::new(io.pins.gpio15, Level::Low);
    let stepper_a_step = Output::new(io.pins.gpio4, Level::Low);
    let stepper_a_en = Output::new(io.pins.gpio16, Level::High);

    let stepper_b_dir = Output::new(io.pins.gpio17, Level::Low);
    let stepper_b_step = Output::new(io.pins.gpio5, Level::Low);
    let stepper_b_en = Output::new(io.pins.gpio18, Level::High);

    let endswitch = Input::new(io.pins.gpio19, Pull::Down);

    let led = Output::new(io.pins.gpio2, Level::Low);

    let control_channel = Channel::<NoopRawMutex, MoveCommand, QUEUE_SIZE>::new();
    let control_channel = Box::leak(Box::new(control_channel));

    let filament_changer = FilamentChanger::new(
        stepper_a_dir,
        stepper_a_step,
        stepper_a_en,
        stepper_b_dir,
        stepper_b_step,
        stepper_b_en,
        endswitch,
        led,
        pwm_pin,
        control_channel.receiver(),
    );

    spawner
        .spawn(filament_changer_task(filament_changer))
        .unwrap();

    loop {
        Timer::after(Duration::from_millis(5_000)).await;
    }
}

/*
Hardware specs:

17HS08-1004S Nema 17 Bipolar 1.8deg 13Ncm (18.4oz.in) 1A 3.5V 42x42x20mm 4 Wires
1.BLK(+)  / 2.GRN(-)
3.RED(+) / 4.BLU(-)

TMC2208 V3.0 Stepper Motor Driver Bigtreetech

From TOP View:
1. GND          9. DIR
2. VIO         10. STEP
3. OB2         11. CLK
4. OB1         12. PDN_UART
5. OA1         13. PDN_UART
6. OA2         14. MS2
7. GND         15. MS1
8. VM          16. EN

MS1 | MS2 | Microstep Resolution
H   | L   | 2
L   | H   | 4
L   | L   | 8
H   | H   | 16


# ESP WROOM 32 Pinout (30 Pin Version)
*/
