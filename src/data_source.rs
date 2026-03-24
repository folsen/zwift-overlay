use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Adapter, Manager};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use uuid::Uuid;

const CYCLING_POWER_MEASUREMENT: Uuid = Uuid::from_u128(0x00002A63_0000_1000_8000_00805f9b34fb);

/// BLE service UUIDs that indicate exercise/fitness devices.
const CYCLING_POWER_SERVICE: Uuid = Uuid::from_u128(0x00001818_0000_1000_8000_00805f9b34fb);
const CYCLING_SPEED_CADENCE_SERVICE: Uuid = Uuid::from_u128(0x00001816_0000_1000_8000_00805f9b34fb);
const FITNESS_MACHINE_SERVICE: Uuid = Uuid::from_u128(0x00001826_0000_1000_8000_00805f9b34fb);
const HEART_RATE_SERVICE: Uuid = Uuid::from_u128(0x0000180D_0000_1000_8000_00805f9b34fb);
const RUNNING_SPEED_CADENCE_SERVICE: Uuid = Uuid::from_u128(0x00001814_0000_1000_8000_00805f9b34fb);

fn is_exercise_device(name: &str, services: &[Uuid]) -> bool {
    let has_exercise_service = services.iter().any(|s| {
        *s == CYCLING_POWER_SERVICE
            || *s == CYCLING_SPEED_CADENCE_SERVICE
            || *s == FITNESS_MACHINE_SERVICE
            || *s == HEART_RATE_SERVICE
            || *s == RUNNING_SPEED_CADENCE_SERVICE
    });
    if has_exercise_service {
        return true;
    }
    // Some devices don't advertise standard service UUIDs in scan responses.
    // Fall back to name-based matching for known exercise equipment brands.
    let lower = name.to_ascii_lowercase();
    const KEYWORDS: &[&str] = &[
        "kickr",
        "wahoo",
        "tacx",
        "elite",
        "stages",
        "quarq",
        "srm",
        "4iiii",
        "favero",
        "assioma",
        "polar",
        "garmin",
        "peloton",
        "saris",
        "cyclops",
        "powertap",
        "rotor",
        "infocrank",
        "power2max",
        "wattbike",
        "concept2",
        "zwift",
        "trainer",
        "power meter",
    ];
    KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// A discovered BLE device.
#[derive(Debug, Clone)]
pub struct BleDevice {
    pub name: String,
    pub id: String,
}

/// Events sent from BLE -> GUI.
#[derive(Debug, Clone)]
pub enum DataEvent {
    Power(f64),
    DeviceList(Vec<BleDevice>),
    Connected(String),
    Disconnected,
    Error(String),
}

/// Commands sent from GUI -> BLE.
#[derive(Debug)]
pub enum BleCommand {
    Connect(String),
    Disconnect,
}

pub async fn run_ble(tx: mpsc::Sender<DataEvent>, mut cmd_rx: mpsc::Receiver<BleCommand>) {
    let manager = match Manager::new().await {
        Ok(m) => m,
        Err(e) => {
            let _ = tx.send(DataEvent::Error(format!("BLE init failed: {e}")));
            return;
        }
    };
    let adapters = match manager.adapters().await {
        Ok(a) => a,
        Err(e) => {
            let _ = tx.send(DataEvent::Error(format!("No BLE adapters: {e}")));
            return;
        }
    };
    let adapter = match adapters.into_iter().next() {
        Some(a) => a,
        None => {
            let _ = tx.send(DataEvent::Error("No BLE adapter found".into()));
            return;
        }
    };

    // Start scanning and keep it running
    let _ = adapter.start_scan(ScanFilter::default()).await;
    let disconnect_flag = Arc::new(AtomicBool::new(false));

    loop {
        // Collect and report current exercise devices
        report_devices(&adapter, &tx).await;

        // Check for connect command, poll for 2 seconds
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            match cmd_rx.try_recv() {
                Ok(BleCommand::Connect(device_id)) => {
                    let _ = adapter.stop_scan().await;
                    disconnect_flag.store(false, Ordering::SeqCst);
                    // Spawn a task to forward Disconnect commands to the flag
                    let flag_for_cmds = disconnect_flag.clone();
                    let cmd_forwarder = tokio::task::spawn_blocking(move || {
                        // Block until we get a Disconnect or the channel closes
                        loop {
                            match cmd_rx.recv_timeout(Duration::from_millis(100)) {
                                Ok(BleCommand::Disconnect) => {
                                    flag_for_cmds.store(true, Ordering::SeqCst);
                                    return Some(cmd_rx);
                                }
                                Ok(BleCommand::Connect(_)) => {
                                    // Ignore connect while already connected
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    if flag_for_cmds.load(Ordering::SeqCst) {
                                        return Some(cmd_rx);
                                    }
                                }
                                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
                            }
                        }
                    });
                    match connect_and_stream(&adapter, &device_id, &tx, &disconnect_flag).await {
                        Ok(()) => {
                            let _ = tx.send(DataEvent::Disconnected);
                        }
                        Err(e) => {
                            let _ = tx.send(DataEvent::Error(format!("{e}")));
                        }
                    }
                    // Recover cmd_rx from the forwarder task
                    disconnect_flag.store(true, Ordering::SeqCst); // ensure forwarder exits
                    match cmd_forwarder.await {
                        Ok(Some(rx)) => cmd_rx = rx,
                        _ => return, // channel closed
                    }
                    // Restart scanning after disconnect
                    let _ = adapter.start_scan(ScanFilter::default()).await;
                    break;
                }
                Ok(BleCommand::Disconnect) => {} // not connected, ignore
                Err(mpsc::TryRecvError::Disconnected) => return,
                Err(mpsc::TryRecvError::Empty) => {}
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

async fn report_devices(adapter: &Adapter, tx: &mpsc::Sender<DataEvent>) {
    let mut devices = Vec::new();
    if let Ok(peripherals) = adapter.peripherals().await {
        for p in &peripherals {
            if let Ok(Some(props)) = p.properties().await {
                if let Some(name) = props.local_name {
                    if !name.is_empty() && is_exercise_device(&name, &props.services) {
                        devices.push(BleDevice {
                            name,
                            id: p.id().to_string(),
                        });
                    }
                }
            }
        }
    }

    let mut seen = HashMap::new();
    devices.retain(|d| seen.insert(d.name.clone(), d.id.clone()).is_none());
    devices.sort_by(|a, b| a.name.cmp(&b.name));

    let _ = tx.send(DataEvent::DeviceList(devices));
}

async fn connect_and_stream(
    adapter: &Adapter,
    device_id: &str,
    tx: &mpsc::Sender<DataEvent>,
    disconnect_flag: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let peripherals = adapter.peripherals().await?;
    let peripheral = peripherals
        .into_iter()
        .find(|p| p.id().to_string() == device_id)
        .ok_or("Device not found")?;

    let name = peripheral
        .properties()
        .await
        .ok()
        .flatten()
        .and_then(|p| p.local_name)
        .unwrap_or_else(|| "Unknown".into());

    peripheral.connect().await?;
    peripheral.discover_services().await?;

    let chars = peripheral.characteristics();
    let power_char = chars
        .iter()
        .find(|c| c.uuid == CYCLING_POWER_MEASUREMENT)
        .ok_or("No Cycling Power Measurement characteristic on this device")?;

    peripheral.subscribe(power_char).await?;
    let _ = tx.send(DataEvent::Connected(name));

    let flag = disconnect_flag.clone();
    let disconnect_wait = async move {
        loop {
            if flag.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    tokio::pin!(disconnect_wait);

    let mut notifications = peripheral.notifications().await?;
    loop {
        tokio::select! {
            maybe_notification = notifications.next() => {
                match maybe_notification {
                    Some(notification) => {
                        if notification.uuid == CYCLING_POWER_MEASUREMENT {
                            if let Some(watts) = parse_cycling_power_measurement(&notification.value) {
                                let _ = tx.send(DataEvent::Power(watts));
                            }
                        }
                    }
                    None => break, // stream ended (device disconnected)
                }
            }
            _ = &mut disconnect_wait => {
                // User requested disconnect
                let _ = peripheral.disconnect().await;
                break;
            }
        }
    }

    Ok(())
}

fn parse_cycling_power_measurement(data: &[u8]) -> Option<f64> {
    if data.len() < 4 {
        return None;
    }
    let watts = i16::from_le_bytes([data[2], data[3]]);
    Some(watts as f64)
}
