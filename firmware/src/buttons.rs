#![allow(clippy::too_many_arguments)]
//! Button matrix scanning implementation
//!
//! This module handles the 3x2 button matrix scanning with debouncing
//! and sends button state changes to the USB task.

use defmt::*;
use embassy_rp::gpio::{Input, Output};
use embassy_time::{Duration, Instant, Timer};

use crate::channels::BUTTON_CHANNEL;
use crate::config::*;
use crate::types::ButtonState;

// ===================================================================
// Button Debouncing State
// ===================================================================

struct ButtonDebouncer {
    buttons: [ButtonDebounceState; 32], // Max keys for any device
}

#[derive(Clone, Copy)]
struct ButtonDebounceState {
    current: bool,
    raw: bool,
    last_change: Instant,
}

impl ButtonDebouncer {
    fn new() -> Self {
        Self {
            buttons: [ButtonDebounceState {
                current: false,
                raw: false,
                last_change: Instant::now(),
            }; 32], // Max keys for any device
        }
    }

    fn update(&mut self, key: usize, raw_state: bool) -> bool {
        let now = Instant::now();
        let state = &mut self.buttons[key];

        if raw_state != state.raw {
            state.raw = raw_state;
            state.last_change = now;
        }

        if now.duration_since(state.last_change) >= Duration::from_millis(BUTTON_DEBOUNCE_MS) {
            let changed = state.current != state.raw;
            state.current = state.raw;
            changed
        } else {
            false
        }
    }

    fn get_state(&self, key: usize) -> bool {
        self.buttons[key].current
    }
}

// ===================================================================
// Button Matrix Scanning
// ===================================================================

struct ButtonMatrix<const ROWS: usize, const COLS: usize> {
    rows: [Output<'static>; ROWS],
    cols: [Input<'static>; COLS],
}

impl<const ROWS: usize, const COLS: usize> ButtonMatrix<ROWS, COLS> {
    fn new(rows: [Output<'static>; ROWS], cols: [Input<'static>; COLS]) -> Self {
        Self { rows, cols }
    }

    async fn scan(&mut self) -> [bool; 32] {
        let mut button_states = [false; 32]; // Max keys for any device

        for row_idx in 0..ROWS {
            // Pull current row low
            self.rows[row_idx].set_low();

            // Small settling time
            Timer::after(Duration::from_micros(10)).await;

            for col_idx in 0..COLS {
                let key_index = row_idx * COLS + col_idx;

                // Read column pin (low = button pressed due to pull-up)
                button_states[key_index] = !self.cols[col_idx].is_high();
            }

            // Return row to high
            self.rows[row_idx].set_high();
        }

        button_states
    }
}

async fn run_matrix_task<const ROWS: usize, const COLS: usize>(
    mut matrix: ButtonMatrix<ROWS, COLS>,
    active_keys: usize,
) {
    let mut debouncer = ButtonDebouncer::new();
    let mut _last_button_state = ButtonState {
        buttons: [false; 32],
        changed: false,
        active_count: active_keys,
    };

    let scan_interval = Duration::from_millis(1000 / BUTTON_SCAN_RATE_HZ);
    let sender = BUTTON_CHANNEL.sender();

    loop {
        // Scan button matrix
        let raw_states = matrix.scan().await;

        // Update debouncer and check for changes
        let mut changed = false;
        let mut new_state = ButtonState::new(active_keys);

        for (i, state) in raw_states.iter().copied().enumerate().take(active_keys) {
            if debouncer.update(i, state) {
                changed = true;
                let pressed = debouncer.get_state(i);
                debug!(
                    "Button {} {}",
                    i,
                    if pressed { "pressed" } else { "released" }
                );
            }
            new_state.set_button(i, debouncer.get_state(i));
        }

        // Send state if changed
        if changed {
            new_state.changed = true;
            sender.send(new_state).await;
            _last_button_state = new_state;
        }

        // Wait for next scan
        Timer::after(scan_interval).await;
    }
}

// ===================================================================
// Button Task Implementation
// ===================================================================

#[embassy_executor::task]
pub async fn button_task_matrix_3x2(
    row0: Output<'static>,
    row1: Output<'static>,
    col0: Input<'static>,
    col1: Input<'static>,
    col2: Input<'static>,
) {
    info!("Button task (matrix 3x2) started");
    let matrix = ButtonMatrix::<2, 3>::new([row0, row1], [col0, col1, col2]);
    run_matrix_task::<2, 3>(matrix, 6).await;
}

#[embassy_executor::task]
#[allow(clippy::too_many_arguments)]
pub async fn button_task_matrix_5x3(
    row0: Output<'static>,
    row1: Output<'static>,
    row2: Output<'static>,
    col0: Input<'static>,
    col1: Input<'static>,
    col2: Input<'static>,
    col3: Input<'static>,
    col4: Input<'static>,
) {
    info!("Button task (matrix 5x3) started");
    let matrix = ButtonMatrix::<3, 5>::new([row0, row1, row2], [col0, col1, col2, col3, col4]);
    run_matrix_task::<3, 5>(matrix, 15).await;
}

#[embassy_executor::task]
#[allow(clippy::too_many_arguments)]
pub async fn button_task_matrix_8x4(
    row0: Output<'static>,
    row1: Output<'static>,
    row2: Output<'static>,
    row3: Output<'static>,
    col0: Input<'static>,
    col1: Input<'static>,
    col2: Input<'static>,
    col3: Input<'static>,
    col4: Input<'static>,
    col5: Input<'static>,
    col6: Input<'static>,
    col7: Input<'static>,
) {
    info!("Button task (matrix 8x4) started");
    let matrix = ButtonMatrix::<4, 8>::new(
        [row0, row1, row2, row3],
        [col0, col1, col2, col3, col4, col5, col6, col7],
    );
    run_matrix_task::<4, 8>(matrix, 32).await;
}

// ===================================================================
// Direct Button Task Implementation
// ===================================================================

#[embassy_executor::task]
pub async fn button_task_direct(inputs: heapless::Vec<Input<'static>, 32>) {
    info!("Button task (direct) started");

    let mut debouncer = ButtonDebouncer::new();
    let mut _last_button_state = ButtonState {
        buttons: [false; 32],
        changed: false,
        active_count: inputs.len(),
    };

    let scan_interval = Duration::from_millis(1000 / BUTTON_SCAN_RATE_HZ);
    let sender = BUTTON_CHANNEL.sender();

    loop {
        // Read all inputs directly (active-low with pull-ups)
        let mut raw_states = [false; 32];
        for (i, pin) in inputs.iter().enumerate() {
            raw_states[i] = !pin.is_high();
        }

        // Debounce and check for changes
        let mut changed = false;
        let active_keys = inputs.len();
        let mut new_state = ButtonState::new(active_keys);

        for (i, state) in raw_states.iter().copied().enumerate().take(active_keys) {
            if debouncer.update(i, state) {
                changed = true;
                let pressed = debouncer.get_state(i);
                debug!(
                    "Button {} {}",
                    i,
                    if pressed { "pressed" } else { "released" }
                );
            }
            new_state.set_button(i, debouncer.get_state(i));
        }

        if changed {
            new_state.changed = true;
            sender.send(new_state).await;
            _last_button_state = new_state;
        }

        Timer::after(scan_interval).await;
    }
}
