use data_url::DataUrl;
use image::load_from_memory_with_format;
use mirajazz::{device::Device, error::MirajazzError, state::DeviceStateUpdate};
use openaction::{OUTBOUND_EVENT_MANAGER, SetImageEvent};
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, TOKENS,
    mappings::{
        COL_COUNT, CandidateDevice, DEVICE_TYPE, ENCODER_COUNT, KEY_COUNT, Kind, ROW_COUNT,
    },
};

/// Initializes a device and listens for events
pub async fn device_task(candidate: CandidateDevice, token: CancellationToken) {
    log::info!("Running device task for {:?}", candidate);

    // Wrap in a closure so we can use `?` operator
    let device = async || -> Result<Device, MirajazzError> {
        let device = connect(&candidate).await?;

        device.set_brightness(50).await?;
        device.clear_all_button_images().await?;
        device.flush().await?;

        Ok(device)
    }()
    .await;

    let device: Device = match device {
        Ok(device) => device,
        Err(err) => {
            handle_error(&candidate.id, err).await;

            log::error!(
                "Had error during device init, finishing device task: {:?}",
                candidate
            );

            return;
        }
    };

    log::info!("Registering device {}", candidate.id);
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        outbound
            .register_device(
                candidate.id.clone(),
                candidate.kind.human_name(),
                ROW_COUNT as u8,
                COL_COUNT as u8,
                ENCODER_COUNT as u8,
                DEVICE_TYPE,
            )
            .await
            .unwrap();
    }

    DEVICES.write().await.insert(candidate.id.clone(), device);

    tokio::select! {
        _ = device_events_task(&candidate) => {},
        _ = token.cancelled() => {}
    };

    log::info!("Shutting down device {:?}", candidate);

    if let Some(device) = DEVICES.read().await.get(&candidate.id) {
        device.shutdown().await.ok();
    }

    log::info!("Device task finished for {:?}", candidate);
}

/// Handles errors, returning true if should continue, returning false if an error is fatal
pub async fn handle_error(id: &String, err: MirajazzError) -> bool {
    log::error!("Device {} error: {}", id, err);

    // Some errors are not critical and can be ignored without sending disconnected event
    if matches!(err, MirajazzError::ImageError(_) | MirajazzError::BadData) {
        return true;
    }

    log::info!("Deregistering device {}", id);
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        outbound.deregister_device(id.clone()).await.unwrap();
    }

    log::info!("Cancelling tasks for device {}", id);
    if let Some(token) = TOKENS.read().await.get(id) {
        token.cancel();
    }

    log::info!("Removing device {} from the list", id);
    DEVICES.write().await.remove(id);

    log::info!("Finished clean-up for {}", id);

    false
}

pub async fn connect(candidate: &CandidateDevice) -> Result<Device, MirajazzError> {
    let result = Device::connect(
        &candidate.dev,
        candidate.kind.protocol_version(),
        KEY_COUNT,
        ENCODER_COUNT,
    )
    .await;

    match result {
        Ok(device) => Ok(device),
        Err(e) => {
            log::error!("Error while connecting to device: {e}");

            Err(e)
        }
    }
}

/// Handles events from device to OpenDeck
async fn device_events_task(candidate: &CandidateDevice) -> Result<(), MirajazzError> {
    log::info!("Connecting to {} for incoming events", candidate.id);

    let devices_lock = DEVICES.read().await;
    let reader = match devices_lock.get(&candidate.id) {
        Some(device) => device.get_reader(crate::inputs::process_input),
        None => return Ok(()),
    };
    drop(devices_lock);

    log::info!("Connected to {} for incoming events", candidate.id);

    log::info!("Reader is ready for {}", candidate.id);

    loop {
        log::info!("Reading updates...");

        let updates = match reader.read(None).await {
            Ok(updates) => updates,
            Err(e) => {
                if !handle_error(&candidate.id, e).await {
                    break;
                }

                continue;
            }
        };

        for update in updates {
            log::debug!("New update: {:#?}", update);

            let id = candidate.id.clone();

            if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                match update {
                    DeviceStateUpdate::ButtonDown(key) => outbound.key_down(id, key).await.unwrap(),
                    DeviceStateUpdate::ButtonUp(key) => outbound.key_up(id, key).await.unwrap(),
                    DeviceStateUpdate::EncoderDown(encoder) => {
                        outbound.encoder_down(id, encoder).await.unwrap();
                    }
                    DeviceStateUpdate::EncoderUp(encoder) => {
                        outbound.encoder_up(id, encoder).await.unwrap();
                    }
                    DeviceStateUpdate::EncoderTwist(encoder, val) => {
                        outbound
                            .encoder_change(id, encoder, val as i16)
                            .await
                            .unwrap();
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handles image setting for buttons and encoder touch zones
pub async fn handle_set_image(device: &Device, evt: SetImageEvent) -> Result<(), MirajazzError> {
    // Check if this is an encoder touch zone or a regular button
    let is_encoder = evt.controller.as_deref() == Some("Encoder");

    if is_encoder {
        // Handle encoder touch zone rendering
        // Hardware has 4 discrete wide LCD buttons (indices 0-3), not a programmable strip
        // Map encoder positions directly to these wide buttons
        match (evt.position, evt.image) {
            (Some(encoder_index), Some(image)) => {
                log::info!("Setting touch zone image for encoder {} (button index {})", encoder_index, encoder_index);

                // OpenDeck sends image as a data URL
                let url = DataUrl::process(image.as_str()).unwrap();
                let (body, _fragment) = url.decode_to_vec().unwrap();

                // Allow only image/jpeg mime type
                if url.mime_type().subtype != "jpeg" {
                    log::error!("Incorrect mime type: {}", url.mime_type());
                    return Ok(()); // Not fatal, just log it
                }

                let image_loaded = load_from_memory_with_format(body.as_slice(), image::ImageFormat::Jpeg)?;

                // Hardware uses button index positioning (discrete LCD buttons, not programmable strip)
                // Tested: write_lcd() is accepted but silently ignored - hardware doesn't support pixel positioning
                let image_format = Kind::from_vid_pid(device.vid, device.pid)
                    .unwrap()
                    .image_format_touchzone();

                device.set_button_image(encoder_index, image_format, image_loaded).await?;
                device.flush().await?;
            }
            (Some(encoder_index), None) => {
                log::info!("Clearing touch zone for encoder {} (button index {})", encoder_index, encoder_index);

                // Clear the wide button at this encoder index
                device.clear_button_image(encoder_index).await?;
                device.flush().await?;
            }
            (None, None) => {
                log::info!("Clearing all touch zones (buttons 0-3)");

                // Clear the 4 wide touch zone buttons (indices 0-3)
                for i in 0..4 {
                    device.clear_button_image(i).await?;
                }
                device.flush().await?;
            }
            _ => {}
        }
    } else {
        // Handle regular button rendering (2x5 grid, positions 0-9)
        // Position correction needed: hardware rows are reversed from OpenDeck layout
        // OpenDeck layout:    Hardware layout:
        // [0] [1] [2] [3] [4]   [10] [11] [12] [13] [14]  <- Top row
        // [5] [6] [7] [8] [9]   [5]  [6]  [7]  [8]  [9]  <- Bottom row

        let corrected_pos = evt.position.map(|pos| {
            match pos {
                0..=4 => pos + 10,  // Top row: OpenDeck 0-4 → Hardware 10-14
                5..=9 => pos,       // Bottom row: OpenDeck 5-9 → Hardware 5-9
                _ => pos,           // Invalid, pass through
            }
        });

        match (corrected_pos, evt.image) {
            (Some(position), Some(image)) => {
                log::info!("Setting image for button {} (OpenDeck pos: {:?})", position, evt.position);

                // OpenDeck sends image as a data URL
                let url = DataUrl::process(image.as_str()).unwrap();
                let (body, _fragment) = url.decode_to_vec().unwrap();

                // Allow only image/jpeg mime type
                if url.mime_type().subtype != "jpeg" {
                    log::error!("Incorrect mime type: {}", url.mime_type());
                    return Ok(()); // Not fatal, just log it
                }

                let image = load_from_memory_with_format(body.as_slice(), image::ImageFormat::Jpeg)?;

                let image_format = Kind::from_vid_pid(device.vid, device.pid)
                    .unwrap()
                    .image_format();

                device.set_button_image(position, image_format, image).await?;
                device.flush().await?;
            }
            (Some(position), None) => {
                device.clear_button_image(position).await?;
                device.flush().await?;
            }
            (None, None) => {
                // Clear all buttons (includes touch zone buttons 0-3 and regular buttons 5-14)
                device.clear_all_button_images().await?;
                device.flush().await?;
            }
            _ => {}
        }
    }

    Ok(())
}
