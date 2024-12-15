use embassy_time::{Duration, Instant, Timer};
use esp_hal::{
    gpio::{AnyPin, Input, Output},
    mcpwm::operator::PwmPin,
    peripherals::MCPWM0,
};

// Servo Motor Limits:
//     300 is min
//     2500 is max
//     0deg is 500, 90deg is 1500, 180deg is 2500
//     90deg is 1500
//     180deg is 2500
const SERVO_RESTING_POSITION: u16 = 900;
const SERVO_CUTTING_POSITION: u16 = 1600;

const HOMING_STEPS: u32 = 2624;

const FILAMENT_START_OFFSET: u32 = 56;
const FILAMENT_DISTANCE: u32 = 800;
const FILAMENT_POSITIONS: [u32; 4] = [
    0 * FILAMENT_DISTANCE + FILAMENT_START_OFFSET,
    1 * FILAMENT_DISTANCE + FILAMENT_START_OFFSET,
    2 * FILAMENT_DISTANCE + FILAMENT_START_OFFSET,
    3 * FILAMENT_DISTANCE + FILAMENT_START_OFFSET,
];

const FILAMENT_RESTING_POSITIONS: [u32; 4] = [
    FILAMENT_POSITIONS[1],
    FILAMENT_POSITIONS[0],
    FILAMENT_POSITIONS[3],
    FILAMENT_POSITIONS[2],
];

const UNLOAD_STEPS: u32 = 14500;
const FAST_LOAD_STEPS: u32 = 15000; // 98mm

fn mm_to_steps(mm: f32) -> u32 {
    (mm * 153.0) as u32
}

const SLOW_LOAD_STEPS: u32 = 12500; // 82mm
const EXTRUDER_FAST_LOAD_STEP_SPEED: Duration = Duration::from_micros(133); // 49.12 mm/s (98mm in 2s)
const EXTRUDER_SLOW_LOAD_STEP_SPEED: Duration = Duration::from_micros(240); // 27.33 mm/s (82mm in 3s)

const EXTRUDER_STEP_SPEED: Duration = Duration::from_micros(100);

const SELECTOR_STEP_SPEED: Duration = Duration::from_micros(500);
const HOMING_STEP_SPEED: Duration = Duration::from_micros(1000);

pub struct FilamentChanger<'a> {
    stepper_a_selector_dir: Output<'a>,
    stepper_a_selector_step: Output<'a>,
    stepper_a_selector_en: Output<'a>,
    stepper_b_extruder_dir: Output<'a>,
    stepper_b_extruder_step: Output<'a>,
    stepper_b_extruder_en: Output<'a>,
    endswitch: Input<'a>,
    led: Output<'a>,
    pwm_pin: PwmPin<'a, AnyPin, MCPWM0, 0, true>,
    current_filament: Option<usize>,
    current_position: u32,
}

impl<'a> FilamentChanger<'a> {
    pub fn new(
        stepper_a_dir: Output<'a>,
        stepper_a_step: Output<'a>,
        stepper_a_en: Output<'a>,
        stepper_b_dir: Output<'a>,
        stepper_b_step: Output<'a>,
        stepper_b_en: Output<'a>,
        endswitch: Input<'a>,
        led: Output<'a>,
        pwm_pin: PwmPin<'a, AnyPin, MCPWM0, 0, true>,
    ) -> Self {
        Self {
            stepper_a_selector_dir: stepper_a_dir,
            stepper_a_selector_step: stepper_a_step,
            stepper_a_selector_en: stepper_a_en,
            stepper_b_extruder_dir: stepper_b_dir,
            stepper_b_extruder_step: stepper_b_step,
            stepper_b_extruder_en: stepper_b_en,
            led,
            endswitch,
            pwm_pin,
            current_filament: None,
            current_position: 0,
        }
    }

    async fn home(&mut self) {
        let start_time = Instant::now();
        log::info!("Homing starting");

        self.change_filament(None).await;

        // disable steppers to save power
        self.stepper_b_extruder_en.set_high();
        self.stepper_a_selector_en.set_high();
        // move servo back to resting position
        self.pwm_pin.set_timestamp(SERVO_RESTING_POSITION);
        Timer::after(Duration::from_millis(2_000)).await;

        let homing_steps_half = HOMING_STEPS / 2;

        // First move: Normal speed
        self.move_stepper_selector(homing_steps_half, false, Some(HOMING_STEP_SPEED))
            .await;
        // Second move: Half speed
        self.move_stepper_selector(homing_steps_half, false, Some(HOMING_STEP_SPEED * 2))
            .await;

        self.current_filament = None;
        self.current_position = 0;

        // disable steppers to save power
        self.stepper_b_extruder_en.set_high();
        self.stepper_a_selector_en.set_high();

        let duration = start_time.elapsed();
        log::info!("Homing completed in {}ms", duration.as_millis());
    }

    async fn move_to_resting_position(&mut self) {
        if let Some(current_filament) = self.current_filament {
            let target_position = FILAMENT_RESTING_POSITIONS[current_filament];
            log::info!(
                "Moving to resting position for filament {}",
                current_filament
            );
            let (steps, direction) = if target_position > self.current_position {
                (target_position - self.current_position, true)
            } else {
                (self.current_position - target_position, false)
            };
            self.move_stepper_selector(steps, direction, None).await;
            self.current_position = target_position;
            log::info!("Moved to resting position: {}", self.current_position);
        } else {
            log::warn!("No current filament selected, cannot move to resting position");
        }
    }

    async fn move_stepper_selector(
        &mut self,
        steps: u32,
        direction: bool,
        speed: Option<Duration>,
    ) {
        self.stepper_b_extruder_en.set_high();
        self.stepper_a_selector_en.set_low();
        if direction {
            self.stepper_a_selector_dir.set_high();
        } else {
            self.stepper_a_selector_dir.set_low();
        }

        let step_speed = speed.unwrap_or(SELECTOR_STEP_SPEED);

        for _ in 0..steps {
            self.step_motor_a(step_speed).await;
        }
    }

    async fn move_stepper_extruder(&mut self, steps: u32, direction: bool, speed: Duration) {
        self.stepper_b_extruder_en.set_low();
        if direction {
            self.stepper_b_extruder_dir.set_high();
        } else {
            self.stepper_b_extruder_dir.set_low();
        }

        for _ in 0..steps {
            self.step_motor_b_extruder(speed).await;
        }
        self.stepper_b_extruder_en.set_high();
    }

    async fn unload_filament(&mut self) {
        self.unload_filament_by(UNLOAD_STEPS, EXTRUDER_STEP_SPEED)
            .await;
    }

    async fn unload_filament_by(&mut self, steps: u32, speed: Duration) {
        if let Some(current_filament) = self.current_filament {
            self.move_stepper_extruder(steps, current_filament < 2, speed)
                .await;
        }
    }

    async fn cut_filament(&mut self) {
        let start_time = Instant::now();

        // disable steppers to save power
        self.stepper_b_extruder_en.set_high();
        self.stepper_a_selector_en.set_high();

        // move servo to cut filament
        self.pwm_pin.set_timestamp(SERVO_CUTTING_POSITION);
        Timer::after(Duration::from_millis(750)).await;
        self.pwm_pin.set_timestamp(SERVO_RESTING_POSITION);

        Timer::after(Duration::from_millis(500)).await;
        self.pwm_pin.set_timestamp(SERVO_CUTTING_POSITION);
        Timer::after(Duration::from_millis(750)).await;
        self.pwm_pin.set_timestamp(SERVO_RESTING_POSITION);

        Timer::after(Duration::from_millis(500)).await;
        self.pwm_pin.set_timestamp(SERVO_CUTTING_POSITION);
        Timer::after(Duration::from_millis(750)).await;
        self.pwm_pin.set_timestamp(SERVO_RESTING_POSITION);
        // move servo back to resting position

        let duration = start_time.elapsed();
        log::info!("Cut completed in {}ms", duration.as_millis());
    }

    async fn load_filament(&mut self) {
        let start_time = Instant::now();
        if let Some(current_filament) = self.current_filament {
            let direction = current_filament >= 2;

            // First section - normal speed
            self.move_stepper_extruder(FAST_LOAD_STEPS, direction, EXTRUDER_FAST_LOAD_STEP_SPEED)
                .await;

            // Second section - slow speed
            self.move_stepper_extruder(SLOW_LOAD_STEPS, direction, EXTRUDER_SLOW_LOAD_STEP_SPEED)
                .await;
        }
        let duration = start_time.elapsed();
        log::info!("load_filament in {}ms", duration.as_millis());
    }

    async fn change_filament(&mut self, new_filament: Option<usize>) {
        if new_filament == self.current_filament {
            log::info!("Filament '{:?}' already selected", new_filament);
            return;
        }

        let start_time = Instant::now();
        log::info!("Selecting filament {:?}", new_filament);
        if let Some(current_filament_id) = self.current_filament {
            self.cut_filament().await;
            self.move_to_filament(current_filament_id).await;
            // // Unload a little bit of filament to reduce the wipe tower size/time
            // self.unload_filament_by(mm_to_steps(10f32), EXTRUDER_STEP_SPEED)
            //     .await;
            self.unload_filament().await;
        }

        if let Some(target_filament_id) = new_filament {
            let start_time_for_change = Instant::now();
            self.move_to_filament(target_filament_id).await;
            // Calculate the maximum possible steps (distance between furthest positions)
            let max_steps = FILAMENT_POSITIONS[3];
            // Calculate and add delay to make all movements take the same time
            let step_time = SELECTOR_STEP_SPEED * 2; // Total time per step (high + low state)
            let max_movement_time = step_time * max_steps;

            log::info!(
                "Took {}ms, max: {}ms",
                start_time_for_change.elapsed().as_millis(),
                max_movement_time.as_millis()
            );

            let elapsed_time = start_time_for_change.elapsed();
            let remaining_time = if elapsed_time < max_movement_time {
                max_movement_time - elapsed_time
            } else {
                Duration::from_micros(0)
            };

            if remaining_time > Duration::from_micros(0) {
                log::debug!(
                    "Adding delay of {:?} to equalize movement time",
                    remaining_time
                );
                Timer::after(remaining_time).await;
            }
            log::info!(
                "time normalized at {}ms",
                start_time_for_change.elapsed().as_millis()
            );

            self.load_filament().await;
            let duration = start_time.elapsed();
            log::info!("Filament changed and loaded in {}ms", duration.as_millis());

            self.move_to_resting_position().await;
        } else {
            self.current_filament = None;
        }
        log::info!("Filament {:?} selected", new_filament);
    }

    async fn move_to_filament(&mut self, filament: usize) {
        let target_position = FILAMENT_POSITIONS[filament];
        log::info!(
            "Moving to filament {}, target position: {}",
            filament,
            target_position
        );

        let (steps, direction) = if target_position > self.current_position {
            log::debug!(
                "Moving forward {} steps",
                target_position - self.current_position
            );
            (target_position - self.current_position, true)
        } else {
            log::debug!(
                "Moving backward {} steps",
                self.current_position - target_position
            );
            (self.current_position - target_position, false)
        };

        log::debug!(
            "Current position: {}, Moving {} steps in direction: {}",
            self.current_position,
            steps,
            direction
        );
        self.move_stepper_selector(steps, direction, None).await;
        self.current_position = target_position;
        self.current_filament = Some(filament);
        log::info!(
            "Moved to filament {}, new position: {}",
            filament,
            self.current_position,
        );
    }

    async fn step_motor_a(&mut self, speed: Duration) {
        self.stepper_a_selector_step.set_high();
        Timer::after(speed).await;
        self.stepper_a_selector_step.set_low();
        Timer::after(speed).await;
    }

    async fn step_motor_b_extruder(&mut self, speed: Duration) {
        self.stepper_b_extruder_step.set_high();
        Timer::after(speed).await;
        self.stepper_b_extruder_step.set_low();
        Timer::after(speed).await;
    }

    pub async fn run(&mut self) {
        log::info!("Starting filament changer");
        self.home().await;

        loop {
            if self.endswitch.is_high() {
                log::debug!("Endswitch triggered");
                self.led.set_low();
                let start = embassy_time::Instant::now();
                let mut last_toggle = start;
                while self.endswitch.is_high() {
                    let now = embassy_time::Instant::now();
                    if (now - last_toggle) >= Duration::from_millis(500) {
                        self.led.toggle();
                        last_toggle = now;
                    }
                    Timer::after(Duration::from_millis(10)).await;
                }
                let duration = start.elapsed();

                if duration >= Duration::from_millis(2750) {
                    log::info!("Homing command detected");
                    self.home().await;
                } else {
                    let filament = match duration.as_millis() {
                        250..=750 => 0,
                        751..=1250 => 1,
                        1251..=1750 => 2,
                        1751..=2250 => 3, // Upper bound to distinguish from homing
                        _ => {
                            log::warn!("Unexpected duration: {} ms", duration.as_millis());
                            continue;
                        }
                    };

                    self.change_filament(Some(filament)).await;
                }
            }

            Timer::after(Duration::from_millis(25)).await;
        }
    }
}
