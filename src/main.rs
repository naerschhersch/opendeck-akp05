use device::DeviceMessage;
use dispatcher::{DISP_TX, dispatcher_task};
use openaction::*;
use std::process::exit;
use tokio::{
    signal::unix::{SignalKind, signal},
    sync::mpsc::{self},
};
use tokio_util::task::TaskTracker;

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
        if let Some(disp_tx) = DISP_TX.lock().await.as_mut() {
            disp_tx
                .send(DeviceMessage::PluginInitialized)
                .await
                .unwrap();
        }

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
        match disp_tx.send(DeviceMessage::ShutdownAll).await {
            Ok(_) => log::info!("Sent shutdown request"),
            Err(err) => log::warn!("Error sending shutdown request: {}", err),
        }
    }
}

async fn connect() {
    if let Err(error) = init_plugin(GlobalEventHandler {}, ActionEventHandler {}).await {
        log::error!("Failed to initialize plugin: {}", error);
        exit(1);
    }
}

async fn sigterm() -> Result<(), Box<dyn std::error::Error>> {
    let mut sig = signal(SignalKind::terminate())?;

    sig.recv().await;

    Ok(())
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

    // A channel for dispatcher thread
    let (disp_tx, disp_rx) = mpsc::channel::<DeviceMessage>(1);

    // Storing dispatcher sender in a global variable
    *DISP_TX.lock().await = Some(disp_tx.clone());

    let tracker = TaskTracker::new();

    tracker.spawn(dispatcher_task(disp_rx, tracker.clone()));

    tokio::select! {
        _ = connect() => {},
        _ = sigterm() => {},
    }

    log::info!("Shutting down");

    shutdown().await;

    log::info!("Waiting for tasks to finish");

    tracker.close();
    tracker.wait().await;

    log::info!("Tasks are finished, exiting now");

    Ok(())
}
