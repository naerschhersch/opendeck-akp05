use data_url::DataUrl;
use image::load_from_memory_with_format;
use mirajazz::{
    device::{Device, list_devices},
    error::MirajazzError,
    state::DeviceStateUpdate,
};
use openaction::SetImageEvent;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender};

use crate::{
    dispatcher::DISP_TX,
    mappings::{
        AJAZZ_VID, CandidateDevice, DEVICE_NAMESPACE, ENCODER_COUNT, IMAGE_FORMAT, KEY_COUNT, Kind,
        MIRABOX_VID,
    },
};

#[derive(Debug)]
pub enum DeviceMessage {
    PluginInitialized,
    SetImage(String, SetImageEvent),
    SetBrightness(String, u8),
    Update(String, DeviceStateUpdate),
    Connected(String, Kind, Sender<DeviceMessage>),
    Disconnected(String),
    ShutdownAll,
}

/// Returns devices that matches known pid/vid pairs
pub async fn get_candidates() -> Result<Vec<CandidateDevice>, MirajazzError> {
    log::info!("Looking for candidate devices");

    let mut candidates: Vec<CandidateDevice> = Vec::new();

    for dev in list_devices(&[AJAZZ_VID, MIRABOX_VID]).await? {
        let id = format!("{}-{}", DEVICE_NAMESPACE, dev.serial_number);

        if let Some(kind) = Kind::from_vid_pid(dev.vid, dev.pid) {
            candidates.push(CandidateDevice {
                id,
                info: dev,
                kind,
            })
        } else {
            continue;
        }
    }

    Ok(candidates)
}

/// Runs tasks for both incoming and outbound events
///
/// Because only outbound events task handles shutdown, uses select to terminate incoming events task
pub async fn device_task(candidate: CandidateDevice) {
    tokio::select! {
        _ = outbound_events_task(&candidate) => {},
        _ = incoming_events_task(&candidate) => {}
    };
}

/// Handles errors, returning true if should continue, returning false if an error is fatal
async fn handle_error(id: &String, err: MirajazzError) -> bool {
    let disp_tx = DISP_TX.lock().await.as_mut().unwrap().clone();

    log::error!("Device {} error: {}", id, err);

    // Some errors are not critical and can be ignored without sending disconnected event
    if matches!(err, MirajazzError::ImageError(_) | MirajazzError::BadData) {
        return true;
    }

    disp_tx
        .send(DeviceMessage::Disconnected(id.clone()))
        .await
        .ok();

    false
}

async fn connect(candidate: &CandidateDevice) -> Result<Device, MirajazzError> {
    Device::connect(
        candidate.info.clone(),
        true,
        candidate.kind.supports_both_states(),
        KEY_COUNT,
        ENCODER_COUNT,
    )
    .await
}

/// Handles events from OpenDeck to device
async fn outbound_events_task(candidate: &CandidateDevice) -> Result<(), MirajazzError> {
    let disp_tx = DISP_TX.lock().await.as_mut().unwrap().clone();

    let device = match connect(candidate).await {
        Ok(device) => device,
        Err(e) => {
            log::error!("Error while connecting to device from outbound task {e}");

            return Err(e);
        }
    };

    log::info!("Connected to {} for outbound events", candidate.id);

    log::info!("Resetting a device");

    device.set_brightness(50).await?;
    device.clear_all_button_images().await?;
    device.flush().await?;

    let (device_tx, mut device_rx) = mpsc::channel::<DeviceMessage>(1);

    disp_tx
        .send(DeviceMessage::Connected(
            candidate.id.clone(),
            candidate.kind.clone(),
            device_tx,
        ))
        .await
        .ok();

    loop {
        let message = match device_rx.recv().await {
            Some(message) => message,
            None => break,
        };

        log::debug!("Device task got message: {:#?}", message);

        let result = match message {
            DeviceMessage::SetImage(_, evt) => handle_set_image(&device, evt).await,
            DeviceMessage::SetBrightness(_, brightness) => device.set_brightness(brightness).await,
            DeviceMessage::ShutdownAll => {
                device.shutdown().await?;

                break;
            }
            _ => Ok(()),
        };

        if let Err(e) = result {
            if !handle_error(&candidate.id, e).await {
                break;
            }
        }
    }

    Ok(())
}

/// Handles events from device to OpenDeck
async fn incoming_events_task(candidate: &CandidateDevice) {
    let disp_tx = DISP_TX.lock().await.as_mut().unwrap().clone();

    let device = match connect(candidate).await {
        Ok(device) => device,
        Err(e) => {
            log::error!("Error while connecting to device from incoming task {e}");

            return;
        }
    };

    log::info!("Connected to {} for incoming events", candidate.id);

    let device = Arc::new(device);
    {
        let reader = device.get_reader(crate::inputs::process_input);

        log::info!("Reader is ready for {}", candidate.id);

        loop {
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

                disp_tx
                    .send(DeviceMessage::Update(candidate.id.clone(), update))
                    .await
                    .ok();
            }
        }

        drop(reader);
    };
}

/// Handles different combinations of "set image" event, including clearing the specific buttons and whole device
async fn handle_set_image(device: &Device, evt: SetImageEvent) -> Result<(), MirajazzError> {
    match (evt.position, evt.image) {
        (Some(position), Some(image)) => {
            // Device has 6 buttons with screens and 3 buttons without screens, so ignore anything above 5
            if position > 5 {
                return Ok(());
            }

            log::info!("Setting image for button {}", position);

            // OpenDeck sends image as a data url, so parse it using a library
            let url = DataUrl::process(image.as_str()).unwrap(); // Isn't expected to fail, so unwrap it is
            let (body, _fragment) = url.decode_to_vec().unwrap(); // Same here

            // Allow only image/jpeg mime for now
            if url.mime_type().subtype != "jpeg" {
                log::error!("Incorrect mime type: {}", url.mime_type());

                return Ok(()); // Not a fatal error, enough to just log it
            }

            let image = load_from_memory_with_format(body.as_slice(), image::ImageFormat::Jpeg)?;

            device
                .set_button_image(position, IMAGE_FORMAT, image)
                .await?;
            device.flush().await?;
        }
        (Some(position), None) => {
            // Device has 6 buttons with screens and 3 buttons without screens, so only clear below 6
            if position > 5 {
                return Ok(());
            }

            device.clear_button_image(position).await?;
            device.flush().await?;
        }
        (None, None) => {
            device.clear_all_button_images().await?;
            device.flush().await?;
        }
        _ => {}
    }

    Ok(())
}
