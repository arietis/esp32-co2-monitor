use crate::display::Ssd1306Display;
use crate::error::AppError;
use crate::sensor::Scd41Sensor;
use esp_idf_svc::hal::i2c::I2cConfig;
use esp_idf_svc::hal::i2c::I2cDriver;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// The device manager interface.
pub struct DeviceManager<'a> {
  /// The SSD1306 display.
  display: Ssd1306Display<'a>,

  /// The SCD-41 sensor.
  sensor: Scd41Sensor<'a>,
}

/// The device manager implementation.
impl<'a> DeviceManager<'a> {
  /// Create a new device manager.
  ///
  /// # Parameters
  /// - `peripherals`: The ESP32 peripherals.
  ///
  /// # Returns
  /// The device manager.
  pub fn new(peripherals: Peripherals) -> Result<Self, AppError> {
    let config = I2cConfig::default().baudrate(100.kHz().into());

    let sda = peripherals.pins.gpio8;

    let scl = peripherals.pins.gpio9;

    let i2c = Rc::new(RefCell::new(
      I2cDriver::new(peripherals.i2c0, sda, scl, &config)
        .map_err(|e| AppError::I2cError(format!("Failed to initialize I2C: {:?}", e)))?
    ));

    // Initialize display
    let mut display = Ssd1306Display::new(Rc::clone(&i2c))?;
    display.init()?;
    display.clear()?;

    // Initialize sensor
    let mut sensor = Scd41Sensor::new(Rc::clone(&i2c))?;
    sensor.start_periodic_measurement()?;

    Ok(Self { display, sensor })
  }

  /// Update the device manager.
  ///
  /// # Returns
  /// The result of the operation.
  pub fn update(&mut self) -> Result<(), AppError> {
    match self.sensor.read_measurement() {
      Ok((co2, temperature, humidity)) => {
        log::info!(
                    "CO2: {} ppm, Temperature: {:.1}°C, Humidity: {:.1}%",
                    co2, temperature, humidity
                );

        if let Err(e) = self.display.draw_measurements(co2, temperature, humidity) {
          log::error!("Failed to update display: {:?}", e);
        }
      }
      Err(e) => {
        log::error!("Failed to read sensor: {:?}", e);
        self.display.draw_error("Sensor Error")?;
      }
    }

    Ok(())
  }
}
