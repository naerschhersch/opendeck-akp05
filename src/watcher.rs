use futures_lite::StreamExt;
use mirajazz::{
    device::{DeviceWatcher, list_devices},
    error::MirajazzError,
    types::{DeviceLifecycleEvent, HidDeviceInfo},
};
use openaction::OUTBOUND_EVENT_MANAGER;
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, TOKENS, TRACKER,
    device::device_task,
    mappings::{CandidateDevice, DEVICE_NAMESPACE, Kind, QUERIES},
};

fn sanitize_identifier(raw: &str, max_len: usize) -> Option<String> {
    let cleaned: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();

    if cleaned.is_empty() {
        None
    } else if cleaned.len() <= max_len {
        Some(cleaned)
    } else {
        Some(cleaned[cleaned.len() - max_len..].to_string())
    }
}

fn normalised_serial(serial: Option<&String>) -> Option<String> {
    serial
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(|s| sanitize_identifier(s, 32))
}

fn fallback_serial(dev: &HidDeviceInfo, kind: &Kind) -> String {
    let mut suffix = format!("{:04X}{:04X}", dev.vendor_id, dev.product_id);

    if let Some(kind_tag) = sanitize_identifier(&format!("{:?}", kind), 8) {
        suffix.push_str(&kind_tag);
    }

    if let Some(id_fragment) = sanitize_identifier(&format!("{:?}", dev.id), 16) {
        suffix.push_str(&id_fragment);
    }

    suffix
}

fn device_id_for(dev: &HidDeviceInfo, kind: &Kind) -> String {
    let suffix =
        normalised_serial(dev.serial_number.as_ref()).unwrap_or_else(|| fallback_serial(dev, kind));

    format!("{}-{}", DEVICE_NAMESPACE, suffix)
}

fn device_info_to_candidate(dev: HidDeviceInfo) -> Option<CandidateDevice> {
    let kind = Kind::from_vid_pid(dev.vendor_id, dev.product_id)?;
    let id = device_id_for(&dev, &kind);

    Some(CandidateDevice { id, dev, kind })
}

fn device_info_to_id(dev: &HidDeviceInfo) -> Option<String> {
    let kind = Kind::from_vid_pid(dev.vendor_id, dev.product_id)?;
    Some(device_id_for(dev, &kind))
}

/// Returns devices that matches known pid/vid pairs
async fn get_candidates() -> Result<Vec<CandidateDevice>, MirajazzError> {
    log::info!("Looking for candidate devices");

    let mut candidates: Vec<CandidateDevice> = Vec::new();

    for dev in list_devices(&QUERIES).await? {
        if let Some(candidate) = device_info_to_candidate(dev.clone()) {
            candidates.push(candidate);
        } else {
            continue;
        }
    }

    Ok(candidates)
}

pub async fn watcher_task(token: CancellationToken) -> Result<(), MirajazzError> {
    let tracker = TRACKER.lock().await.clone();

    // Scans for connected devices that (possibly) we can use
    let candidates = get_candidates().await?;

    log::info!("Looking for connected devices");

    for candidate in candidates {
        log::info!("New candidate {:#?}", candidate);

        let token = CancellationToken::new();

        TOKENS
            .write()
            .await
            .insert(candidate.id.clone(), token.clone());

        tracker.spawn(device_task(candidate, token));
    }

    let mut watcher = DeviceWatcher::new();
    let mut watcher_stream = watcher.watch(&QUERIES).await?;

    log::info!("Watcher is ready");

    loop {
        let ev = tokio::select! {
            v = watcher_stream.next() => v,
            _ = token.cancelled() => None
        };

        if let Some(ev) = ev {
            log::info!("New device event: {:?}", ev);

            match ev {
                DeviceLifecycleEvent::Connected(info) => {
                    if let Some(candidate) = device_info_to_candidate(info) {
                        // Don't add existing device again
                        if DEVICES.read().await.contains_key(&candidate.id) {
                            continue;
                        }

                        let token = CancellationToken::new();

                        TOKENS
                            .write()
                            .await
                            .insert(candidate.id.clone(), token.clone());

                        log::debug!("Spawning task for new device: {:?}", candidate);
                        tracker.spawn(device_task(candidate, token));
                        log::debug!("Spawned");
                    }
                }
                DeviceLifecycleEvent::Disconnected(info) => {
                    let Some(id) = device_info_to_id(&info) else {
                        log::warn!(
                            "Disconnected unknown device (VID {:04X} PID {:04X})",
                            info.vendor_id,
                            info.product_id
                        );
                        continue;
                    };

                    if let Some(token) = TOKENS.write().await.remove(&id) {
                        log::info!("Sending cancel request for {}", id);
                        token.cancel();
                    }

                    DEVICES.write().await.remove(&id);

                    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                        outbound.deregister_device(id.clone()).await.ok();
                    }

                    log::info!("Disconnected device {}", id);
                }
            }
        } else {
            log::info!("Watcher is shutting down");

            break Ok(());
        }
    }
}
