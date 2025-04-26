use std::{process::exit, sync::Arc, thread::sleep, time::Duration};

use data_url::DataUrl;
use image::load_from_memory_with_format;
use mirajazz::{
    device::{Device, list_devices, new_hidapi},
    state::DeviceStateUpdate,
};
use openaction::SetImageEvent;
use tokio::sync::mpsc::{self, Sender};

use crate::mappings::{
    AJAZZ_VID, CandidateDevice, ENCODER_COUNT, IMAGE_FORMAT, KEY_COUNT, Kind, MIRABOX_VID,
};

const POLL_RATE_MS: u64 = 50;

#[derive(Debug)]
pub enum DeviceMessage {
    SetImage(String, SetImageEvent),
    SetBrightness(String, u8),
    Update(String, DeviceStateUpdate),
    Connected(String, Kind, Sender<DeviceMessage>),
    Disconnected(String),
}

/// Returns devices that matches known pid/vid pairs
pub fn get_candidates() -> Vec<CandidateDevice> {
    log::info!("Looking for candidate devices");

    let hidapi = match new_hidapi() {
        Ok(hidapi) => hidapi,
        Err(e) => {
            log::error!("Failed to init hidapi: {e}");
            exit(1);
        }
    };

    let mut candidates: Vec<CandidateDevice> = Vec::new();

    for (vid, pid, serial) in list_devices(&hidapi, &[AJAZZ_VID, MIRABOX_VID]) {
        let id = format!("n3-{}", serial);

        if let Some(kind) = Kind::from_vid_pid(vid, pid) {
            candidates.push(CandidateDevice {
                id,
                vid,
                pid,
                serial,
                kind,
            })
        } else {
            continue;
        }
    }

    candidates
}

/// Runs in a separate thread, handling events bound to a device
pub fn device_task(candidate: CandidateDevice, disp_tx: Sender<DeviceMessage>) {
    let (device_tx, mut device_rx) = mpsc::channel::<DeviceMessage>(1);

    let hidapi = match new_hidapi() {
        Ok(hidapi) => hidapi,
        Err(e) => {
            log::error!("Failed to init hidapi: {e}");
            exit(1);
        }
    };

    log::info!("Connecting to {} from a device task", candidate.id);

    let device = Device::connect(
        &hidapi,
        candidate.vid,
        candidate.pid,
        &candidate.serial,
        true,
        KEY_COUNT,
        ENCODER_COUNT,
    )
    .expect("Failed to connect");

    log::info!("Connected to {}", candidate.id);

    log::info!("Resetting a device");

    device.set_brightness(100).unwrap();
    device.clear_all_button_images().unwrap();

    device.flush().unwrap();

    disp_tx
        .blocking_send(DeviceMessage::Connected(
            candidate.id.clone(),
            candidate.kind.clone(),
            device_tx,
        ))
        .unwrap();

    let device = Arc::new(device);
    {
        let reader = device.get_reader();

        log::debug!("Reading events from {}", candidate.id);

        loop {
            if let Ok(message) = device_rx.try_recv() {
                log::debug!("Device task got message: {:#?}", message);

                match message {
                    DeviceMessage::SetImage(_, evt) => {
                        handle_set_image(&device, evt);
                    }
                    DeviceMessage::SetBrightness(_, brightness) => {
                        device.set_brightness(brightness).unwrap();
                    }
                    _ => {}
                }
            }

            match reader.read(
                None,
                crate::inputs::process_input,
                candidate.kind.supports_both_states(),
            ) {
                Ok(updates) => {
                    for update in updates {
                        log::debug!("New update: {:#?}", update);

                        disp_tx
                            .blocking_send(DeviceMessage::Update(candidate.id.clone(), update))
                            .unwrap();
                    }
                }
                Err(err) => {
                    log::error!("Device error {}: {}", candidate.id, err);

                    disp_tx
                        .blocking_send(DeviceMessage::Disconnected(candidate.id.clone()))
                        .unwrap();

                    break;
                }
            };

            sleep(Duration::from_millis(POLL_RATE_MS));
        }

        drop(reader);
    }
}

/// Handles different combinations of "set image" event, including clearing the specific buttons and whole device
fn handle_set_image(device: &Device, evt: SetImageEvent) {
    match (evt.position, evt.image) {
        (Some(position), Some(image)) => {
            // Device has 6 buttons with screens and 3 buttons without screens, so ignore anything above 5
            if position > 5 {
                return;
            }

            log::info!("Setting image for button {}", position);

            // OpenDeck sends image as a data urls, so parse them
            let url = DataUrl::process(image.as_str()).unwrap();
            let (body, _fragment) = url.decode_to_vec().unwrap();

            // Allow only image/jpeg mime for now
            if url.mime_type().subtype != "jpeg" {
                log::error!("Incorrect image type: {}", url.mime_type().subtype);

                return;
            }

            let image =
                load_from_memory_with_format(body.as_slice(), image::ImageFormat::Jpeg).unwrap();

            device
                .set_button_image(position, IMAGE_FORMAT, image)
                .unwrap();

            device.flush().unwrap();
        }
        (Some(position), None) => {
            // Device has 6 buttons with screens and 3 buttons without screens, so only clear below 6
            if position < 6 {
                device.clear_button_image(position).unwrap();

                device.flush().unwrap();
            }
        }
        (None, None) => {
            device.clear_all_button_images().unwrap();

            device.flush().unwrap();
        }
        _ => {}
    }
}
