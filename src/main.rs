mod font;
mod display;
mod sensor;
mod error;
mod device;

use crate::device::DeviceManager;
use crate::error::AppError;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::sys::link_patches;
use log::info;

/// Measurement interval in milliseconds.
const MEASUREMENT_INTERVAL_MS: u32 = 5000;

/// This function initializes the system and starts the main loop.
///
/// # Returns
/// The result of the operation.
fn main() -> Result<(), AppError> {
  // Initialize system
  link_patches();
  EspLogger::initialize_default();
  info!("Starting the SCD-41 sensor application...");

  let peripherals = Peripherals::take()
    .map_err(|_| AppError::PeripheralsError("Failed to acquire ESP32 peripherals".into()))?;

  // Initialize device manager
  let mut manager = DeviceManager::new(peripherals)?;

  // Main loop
  loop {
    manager.update()?;
    FreeRtos::delay_ms(MEASUREMENT_INTERVAL_MS);
  }
}
