use data_url::DataUrl;
use image::{DynamicImage, GenericImageView, load_from_memory_with_format};
use mirajazz::{device::Device, error::MirajazzError, state::DeviceStateUpdate};
use openaction::{OUTBOUND_EVENT_MANAGER, SetImageEvent};
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, TOKENS,
    mappings::{
        COL_COUNT, CandidateDevice, ENCODER_COUNT, KEY_COUNT, Kind, ROW_COUNT, TOUCH_ZONES,
    },
};

// Hardware index mapping: adjust if testing reveals a different device order.
const TOUCH_INDEX_MAP: [u8; ENCODER_COUNT] = [0, 1, 2, 3];
const BUTTON_INDEX_MAP: [u8; KEY_COUNT] = [8, 7, 6, 5, 4, 9, 10, 11, 12, 13];

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

    let reg_id = candidate.id.clone();
    let reg_name = candidate.kind.human_name();
    let reg_rows = ROW_COUNT as u8;
    let reg_cols = COL_COUNT as u8;
    let reg_encoders = ENCODER_COUNT as u8;
    let reg_touch = TOUCH_ZONES as u8;

    log::info!(
        "Registering device id={} name=\"{}\" rows={} cols={} encoders={} touch_zones={}",
        reg_id,
        reg_name,
        reg_rows,
        reg_cols,
        reg_encoders,
        reg_touch
    );

    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        outbound
            .register_device(
                reg_id,
                reg_name,
                reg_rows,
                reg_cols,
                reg_encoders,
                reg_touch,
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

/// Handles different combinations of "set image" event, including clearing the specific buttons and whole device
pub async fn handle_set_image(device: &Device, evt: SetImageEvent) -> Result<(), MirajazzError> {
    let kind = match Kind::from_vid_pid(device.vid, device.pid) {
        Some(kind) => kind,
        None => {
            log::warn!(
                "Unable to determine device kind for image update (VID {:04X}, PID {:04X})",
                device.vid,
                device.pid
            );
            return Ok(());
        }
    };

    let mut target_is_touch = matches!(evt.controller.as_deref(), Some("Encoder"));

    if let Some(pos) = evt.position {
        if (pos as usize) < ENCODER_COUNT {
            target_is_touch = true;
        }
    }

    let image = if let Some(data) = evt.image.as_ref() {
        let (body, fmt) = decode_image_data(data)?;
        Some(load_from_memory_with_format(body.as_slice(), fmt)?)
    } else {
        None
    };

    if target_is_touch {
        handle_touch_strip_image(device, kind, evt.position, image).await?;
    } else {
        handle_button_images(device, kind, evt.position, image).await?;
    }

    Ok(())
}

async fn handle_button_images(
    device: &Device,
    kind: Kind,
    position: Option<u8>,
    image: Option<DynamicImage>,
) -> Result<(), MirajazzError> {
    match (position, image) {
        (Some(pos), Some(image)) => {
            if let Some(&index) = BUTTON_INDEX_MAP.get(pos as usize) {
                device
                    .set_button_image(index, kind.image_format(), image)
                    .await?;
                device.flush().await?;
            } else {
                log::warn!(
                    "Ignoring button image for out-of-range logical position {}",
                    pos
                );
            }
        }
        (Some(pos), None) => {
            if let Some(&index) = BUTTON_INDEX_MAP.get(pos as usize) {
                device.clear_button_image(index).await?;
                device.flush().await?;
            } else {
                log::warn!(
                    "Ignoring button clear for out-of-range logical position {}",
                    pos
                );
            }
        }
        (None, None) => {
            for &index in BUTTON_INDEX_MAP.iter() {
                device.clear_button_image(index).await?;
            }
            device.flush().await?;
        }
        (None, Some(image)) => {
            for &index in BUTTON_INDEX_MAP.iter() {
                device
                    .set_button_image(index, kind.image_format(), image.clone())
                    .await?;
            }
            device.flush().await?;
        }
    }

    Ok(())
}

async fn handle_touch_strip_image(
    device: &Device,
    kind: Kind,
    position: Option<u8>,
    image: Option<DynamicImage>,
) -> Result<(), MirajazzError> {
    let touch_format = kind.touch_image_format();

    match (position, image) {
        (Some(pos), Some(image)) => {
            if let Some(&index) = TOUCH_INDEX_MAP.get(pos as usize) {
                device.set_button_image(index, touch_format, image).await?;
                device.flush().await?;
            } else {
                log::warn!(
                    "Ignoring touch image update for out-of-range position {}",
                    pos
                );
            }
        }
        (Some(pos), None) => {
            if let Some(&index) = TOUCH_INDEX_MAP.get(pos as usize) {
                device.clear_button_image(index).await?;
                device.flush().await?;
            } else {
                log::warn!(
                    "Ignoring touch clear request for out-of-range position {}",
                    pos
                );
            }
        }
        (None, None) => {
            for &index in TOUCH_INDEX_MAP.iter() {
                device.clear_button_image(index).await?;
            }
            device.flush().await?;
        }
        (None, Some(full_image)) => {
            let width = full_image.width();
            let height = full_image.height();

            if width == 0 || height == 0 {
                log::warn!("Received empty encoder strip image ({}x{})", width, height);
                return Ok(());
            }

            let zones = TOUCH_INDEX_MAP.len();
            if zones == 0 {
                return Ok(());
            }

            let chunk_width = (width / zones as u32).max(1);

            for (zone, &index) in TOUCH_INDEX_MAP.iter().enumerate() {
                let x = chunk_width * zone as u32;
                if x >= width {
                    break;
                }

                let remaining = width.saturating_sub(x);
                let segment_width = if zone == zones - 1 {
                    remaining
                } else {
                    chunk_width.min(remaining)
                };

                if segment_width == 0 {
                    continue;
                }

                let segment = full_image.crop_imm(x, 0, segment_width, height);

                device
                    .set_button_image(index, touch_format, segment)
                    .await?;
            }

            device.flush().await?;
        }
    }

    Ok(())
}

fn decode_image_data(data: &str) -> Result<(Vec<u8>, image::ImageFormat), MirajazzError> {
    let url = match DataUrl::process(data) {
        Ok(url) => url,
        Err(err) => {
            log::error!("Failed to parse image data URL: {}", err);
            return Err(MirajazzError::BadData);
        }
    };

    let (body, _fragment) = match url.decode_to_vec() {
        Ok(decoded) => decoded,
        Err(err) => {
            log::error!("Failed to decode image payload: {}", err);
            return Err(MirajazzError::BadData);
        }
    };

    let format = match url.mime_type().subtype.as_str() {
        "jpeg" | "jpg" => image::ImageFormat::Jpeg,
        "png" => image::ImageFormat::Png,
        other => {
            log::error!("Unsupported image mime type: {}", other);
            return Err(MirajazzError::BadData);
        }
    };

    Ok((body, format))
}
