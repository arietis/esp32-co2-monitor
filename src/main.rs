mod ble;
mod device;
mod display;
mod error;
mod font;
mod sensor;

use crate::{device::DeviceManager, error::AppError};
use esp_idf_svc::{
    hal::{delay::FreeRtos, peripherals::Peripherals},
    log::EspLogger,
    sys::link_patches,
};
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
        .map_err(|e| AppError::PeripheralsError(format!("Failed to take peripherals: {:?}", e)))?;
    info!("Peripherals taken!");
    // Initialize device manager
    let mut manager = DeviceManager::new(peripherals)?;
    info!("Manager created!");
    // Main loop
    loop {
        manager.update()?;
        info!("Manager updated!");
        FreeRtos::delay_ms(MEASUREMENT_INTERVAL_MS);
        info!("Delay done!");
    }
}
