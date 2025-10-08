use mirajazz::{error::MirajazzError, types::DeviceInput};

use crate::mappings::{ENCODER_COUNT, KEY_COUNT};

// TODO: These input mappings are placeholders and need to be verified with the actual AKP05 device
// The actual input codes will need to be discovered by testing with the real hardware
//
// Note: The touchscreen zones belong to encoders, not buttons. Similar to Stream Deck+,
// each encoder has an associated touch zone on the touchscreen strip.
// OpenDeck handles the touchscreen rendering and swipe-to-switch-page functionality automatically.

pub fn process_input(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    log::debug!("Processing input: 0x{:02X}, state: {}", input, state);

    match input {
        // Physical LCD buttons (1-10)
        // TODO: Update button range for 10 buttons (AKP05 has 10 vs AKP03's 9)
        (0..=10) => read_button_press(input, state),

        // Touchscreen tap events (4 zones, one per encoder)
        // TODO: Discover actual input codes for touchscreen zones and verify handling mechanism
        // These are placeholders that need to be verified with real hardware
        //
        // Note: Touch zones belong to encoders (similar to Stream Deck+). OpenDeck handles
        // touchscreen rendering and swipe-to-switch-page automatically. Touch tap events might:
        // 1. Be sent as separate touch events (if mirajazz/openaction add support), OR
        // 2. Be handled internally by the device firmware, OR
        // 3. Need to be discovered during hardware testing
        0x40..=0x43 => {
            log::warn!("Touch input code received: 0x{:02X} - handling mechanism needs verification", input);
            // For now, return error to avoid crashes. Update after hardware testing.
            Err(MirajazzError::BadData)
        }

        // Encoder rotation (4 encoders)
        // TODO: Verify these codes with actual hardware
        0x90 | 0x91 | 0x50 | 0x51 | 0x60 | 0x61 | 0x70 | 0x71 => read_encoder_value(input),

        // Encoder press (4 encoders)
        // TODO: Verify these codes with actual hardware
        0x33..=0x36 => read_encoder_press(input, state),

        _ => {
            log::warn!("Unknown input code: 0x{:02X}, state: {}", input, state);
            Err(MirajazzError::BadData)
        }
    }
}

fn read_button_states(states: &[u8]) -> Vec<bool> {
    let mut bools = vec![];

    for i in 0..KEY_COUNT {
        bools.push(states[i + 1] != 0);
    }

    bools
}

fn read_button_press(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    let mut button_states = vec![0x01];
    button_states.extend(vec![0u8; KEY_COUNT + 1]);

    if input == 0 {
        return Ok(DeviceInput::ButtonStateChange(read_button_states(
            &button_states,
        )));
    }

    // TODO: Map actual AKP05 input codes to button indices (1-10)
    // This is a placeholder mapping that needs to be verified with real hardware
    let pressed_index: usize = match input {
        (1..=10) => input as usize,  // 10 buttons for AKP05
        _ => return Err(MirajazzError::BadData),
    };

    button_states[pressed_index] = state;

    Ok(DeviceInput::ButtonStateChange(read_button_states(
        &button_states,
    )))
}

fn read_encoder_value(input: u8) -> Result<DeviceInput, MirajazzError> {
    let mut encoder_values = vec![0i8; ENCODER_COUNT];

    // TODO: Verify these encoder rotation codes with actual AKP05 hardware
    // Added 4th encoder (0x70/0x71) compared to AKP03 which only had 3
    let (encoder, value): (usize, i8) = match input {
        // Encoder 1
        0x90 => (0, -1),
        0x91 => (0, 1),
        // Encoder 2
        0x50 => (1, -1),
        0x51 => (1, 1),
        // Encoder 3
        0x60 => (2, -1),
        0x61 => (2, 1),
        // Encoder 4 (new for AKP05)
        0x70 => (3, -1),
        0x71 => (3, 1),
        _ => return Err(MirajazzError::BadData),
    };

    encoder_values[encoder] = value;
    Ok(DeviceInput::EncoderTwist(encoder_values))
}

fn read_encoder_press(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
    let mut encoder_states = vec![false; ENCODER_COUNT];

    // TODO: Verify these encoder press codes with actual AKP05 hardware
    // Added 4th encoder (0x36) compared to AKP03 which only had 3
    let encoder: usize = match input {
        0x33 => 0, // Encoder 1
        0x35 => 1, // Encoder 2
        0x34 => 2, // Encoder 3
        0x36 => 3, // Encoder 4 (new for AKP05)
        _ => return Err(MirajazzError::BadData),
    };

    encoder_states[encoder] = state != 0;
    Ok(DeviceInput::EncoderStateChange(encoder_states))
}

// DEPRECATED: Touch zones are now handled as part of the encoder system, not as separate buttons.
// This function is kept for reference during hardware testing.
// When register_device() is called with touch_zones > 0, OpenDeck handles touchscreen automatically.
//
// fn read_touch_press(input: u8, state: u8) -> Result<DeviceInput, MirajazzError> {
//     // OLD IMPLEMENTATION: Treated touchscreen zones as additional buttons (indices 11-14)
//     let mut button_states = vec![0x01];
//     button_states.extend(vec![0u8; KEY_COUNT + TOUCH_ZONES + 1]);
//
//     if input == 0 {
//         return Ok(DeviceInput::ButtonStateChange(read_button_states(
//             &button_states,
//         )));
//     }
//
//     let pressed_index: usize = match input {
//         0x40 => KEY_COUNT + 1, // Touch zone 1
//         0x41 => KEY_COUNT + 2, // Touch zone 2
//         0x42 => KEY_COUNT + 3, // Touch zone 3
//         0x43 => KEY_COUNT + 4, // Touch zone 4
//         _ => return Err(MirajazzError::BadData),
//     };
//
//     button_states[pressed_index] = state;
//
//     Ok(DeviceInput::ButtonStateChange(read_button_states(
//         &button_states,
//     )))
// }
