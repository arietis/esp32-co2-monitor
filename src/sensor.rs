use crate::error::AppError;
use esp_idf_svc::hal::{delay::FreeRtos, i2c::I2cDriver};
use log::info;
use scd41_core::scd41::parse_measurement;
use std::{cell::RefCell, rc::Rc};

/// Command to read measurement.
const CMD_READ_MEASUREMENT: u16 = 0xec05;

/// Command to start periodic measurement.
const CMD_START_PERIODIC_MEASUREMENT: u16 = 0x21b1;

/// Command to stop periodic measurement.
const CMD_STOP_PERIODIC_MEASUREMENT: u16 = 0x3f86;

/// SCD41 I2C address.
const SCD41_ADDRESS: u8 = 0x62;

/// SCD41 sensor interface.
pub struct Scd41Sensor<'a> {
    /// The I2C driver.
    i2c: Rc<RefCell<I2cDriver<'a>>>,
}

/// The SCD41 sensor implementation.
impl<'a> Scd41Sensor<'a> {
    /// Create a new SCD41 sensor.
    ///
    /// # Parameters
    /// - `i2c`: The I2C driver.
    ///
    /// # Returns
    /// The SCD41 sensor.
    pub fn new(i2c: Rc<RefCell<I2cDriver<'a>>>) -> Result<Self, AppError> {
        info!("Waiting for SCD41 sensor to become ready after power-on...");
        FreeRtos::delay_ms(1000);
        Ok(Self { i2c })
    }

    /// Start periodic measurement.
    ///
    /// # Returns
    /// The result of the operation.
    pub fn start_periodic_measurement(&mut self) -> Result<(), AppError> {
        let mut i2c = self.i2c.borrow_mut();
        self.send_command(&mut i2c, CMD_START_PERIODIC_MEASUREMENT)?;
        FreeRtos::delay_ms(5000);

        Ok(())
    }

    /// Stop periodic measurement.
    ///
    /// # Returns
    /// The result of the operation.
    pub fn stop_periodic_measurement(&mut self) -> Result<(), AppError> {
        let mut i2c = self.i2c.borrow_mut();
        self.send_command(&mut i2c, CMD_STOP_PERIODIC_MEASUREMENT)?;
        FreeRtos::delay_ms(500);

        Ok(())
    }

    /// Read measurement.
    ///
    /// # Returns
    /// The measurement.
    pub fn read_measurement(&mut self) -> Result<(u16, f32, f32), AppError> {
        let mut i2c = self.i2c.borrow_mut();
        self.send_command(&mut i2c, CMD_READ_MEASUREMENT)?;
        FreeRtos::delay_ms(1);

        let mut buffer = [0u8; 9];
        i2c.read(SCD41_ADDRESS, &mut buffer, 100).map_err(|e| {
            AppError::SensorError(format!(
                "Failed to read measurement data from sensor at address 0x{:02x}: {:?}",
                SCD41_ADDRESS, e
            ))
        })?;

        let measurement = parse_measurement(&buffer).map_err(|e| {
            AppError::SensorError(format!(
                "Failed to parse measurement data from sensor at address 0x{:02x}: {e}",
                SCD41_ADDRESS
            ))
        })?;

        Ok((
            measurement.co2_ppm,
            measurement.temperature_c,
            measurement.humidity_percent,
        ))
    }

    /// Send a command to the sensor.
    ///
    /// # Parameters
    /// - `i2c`: The I2C driver.
    /// - `command`: The command.
    ///
    /// # Returns
    /// The result of the operation.
    fn send_command(&self, i2c: &mut I2cDriver<'a>, command: u16) -> Result<(), AppError> {
        let bytes = command.to_be_bytes();
        i2c.write(SCD41_ADDRESS, &bytes, 100).map_err(|e| {
            AppError::SensorError(format!(
                "Failed to send command 0x{:04x} to sensor at address 0x{:02x}: {:?}",
                command, SCD41_ADDRESS, e
            ))
        })
    }
}

/// Implement the `Drop` trait for `Scd41Sensor`.
impl<'a> Drop for Scd41Sensor<'a> {
    /// Stop periodic measurement when the sensor is dropped.
    fn drop(&mut self) {
        // Try to stop measurements when the sensor is dropped
        let _ = self.stop_periodic_measurement();
    }
}
