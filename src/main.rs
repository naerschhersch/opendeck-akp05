use device::{DeviceMessage, device_task, get_candidates};
use dispatcher::{DISP_TX, dispatcher_task};
use openaction::*;
use std::thread;
use std::time::Duration;
use std::{process::exit, thread::sleep};
use tokio::sync::mpsc::{self};

mod device;
mod dispatcher;
mod inputs;
mod mappings;

struct GlobalEventHandler {}
impl openaction::GlobalEventHandler for GlobalEventHandler {
    async fn plugin_ready(
        &self,
        _outbound: &mut openaction::OutboundEventManager,
    ) -> EventHandlerResult {
        // A channel for dispatcher thread
        let (disp_tx, disp_rx) = mpsc::channel::<DeviceMessage>(1);

        // Storing dispatcher sender in a global variable
        *DISP_TX.lock().await = Some(disp_tx.clone());

        tokio::task::spawn(dispatcher_task(disp_rx));

        // Scans for connected devices that (possibly) we can use
        let candidates = get_candidates();

        for device in candidates {
            log::info!("New candidate {:#?}", device);

            let disp_tx = disp_tx.clone();

            // Spawn a separate thread for each device
            thread::spawn(move || device_task(device, disp_tx));
        }

        log::info!("Finished init");

        Ok(())
    }

    async fn set_image(
        &self,
        event: SetImageEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        log::debug!("Asked to set image: {:#?}", event);

        let id = event.device.clone();

        if let Some(disp_tx) = DISP_TX.lock().await.as_mut() {
            disp_tx
                .send(DeviceMessage::SetImage(id, event.clone()))
                .await
                .unwrap();
        } else {
            log::error!("Received event for unknown device: {}", event.device);
        }

        Ok(())
    }

    async fn set_brightness(
        &self,
        event: SetBrightnessEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        log::debug!("Asked to set brightness: {:#?}", event);

        let id = event.device.clone();

        if let Some(disp_tx) = DISP_TX.lock().await.as_mut() {
            disp_tx
                .send(DeviceMessage::SetBrightness(id, event.brightness))
                .await
                .unwrap();
        } else {
            log::error!("Received event for unknown device: {}", event.device);
        }

        Ok(())
    }
}

struct ActionEventHandler {}
impl openaction::ActionEventHandler for ActionEventHandler {}

async fn shutdown() {
    if let Some(disp_tx) = DISP_TX.lock().await.as_mut() {
        disp_tx.send(DeviceMessage::ShutdownAll).await.unwrap();
    }

    // Allow threads to finish
    sleep(Duration::from_millis(2000));
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Debug,
        simplelog::Config::default(),
        simplelog::TerminalMode::Stdout,
        simplelog::ColorChoice::Never,
    )
    .unwrap();

    if let Err(error) = init_plugin(GlobalEventHandler {}, ActionEventHandler {}).await {
        log::error!("Failed to initialize plugin: {}", error);
        exit(1);
    }

    log::info!("Shutting down...");

    shutdown().await;

    Ok(())
}
