use crate::{ble::BleServer, display::Ssd1306Display, error::AppError, sensor::Scd41Sensor};
use esp_idf_svc::{
    hal::{
        i2c::{I2cConfig, I2cDriver},
        peripherals::Peripherals,
        units::Hertz,
    },
    nvs::{EspNvsPartition, NvsDefault},
};
use log::{error, info};
use std::{cell::RefCell, rc::Rc};

/// The device manager interface.
pub struct DeviceManager<'a> {
    /// The BLE server.
    ble: Option<BleServer>,

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
        info!("Initializing device manager");

        let config = I2cConfig::default().baudrate(Hertz(100000));

        let mut led = esp_idf_svc::hal::gpio::PinDriver::output(peripherals.pins.gpio8)?;
        led.set_low()?;
        std::mem::forget(led);

        let i2c = Rc::new(RefCell::new(
            I2cDriver::new(
                peripherals.i2c0,
                peripherals.pins.gpio4,
                peripherals.pins.gpio5,
                &config,
            )
            .map_err(|e| AppError::I2cError(format!("Failed to initialize I2C: {:?}", e)))?,
        ));

        // Initialize display
        let mut display = Ssd1306Display::new(Rc::clone(&i2c))?;
        display.init()?;
        display.clear()?;

        // Initialize sensor
        let mut sensor = Scd41Sensor::new(Rc::clone(&i2c))?;
        sensor.start_periodic_measurement()?;
        info!("Sensor and display ready!");

        // Initialize BLE if available
        let ble = if let Ok(nvs) = EspNvsPartition::<NvsDefault>::take() {
            match BleServer::new(peripherals.modem, Some(nvs)) {
                Ok(server) => Some(server),
                Err(e) => {
                    error!("Failed to initialize BLE: {:?}", e);
                    None
                }
            }
        } else {
            error!("Failed to initialize NVS partition");
            None
        };
        info!("BLE server ready!");

        Ok(Self {
            ble,
            display,
            sensor,
        })
    }

    /// Update the device manager.
    ///
    /// # Returns
    /// The result of the operation.
    pub fn update(&mut self) -> Result<(), AppError> {
        match self.sensor.read_measurement() {
            Ok((co2, temp_value, humidity_value)) => {
                info!(
                    "CO2: {} ppm, Temperature: {:.2} °C, Humidity: {:.2} %",
                    co2, temp_value, humidity_value
                );

                if let Err(e) = self
                    .display
                    .draw_measurements(co2, temp_value, humidity_value)
                {
                    error!("Failed to update display: {:?}", e);
                }

                if let Some(ble_server) = &self.ble {
                    ble_server.update_values(
                        (temp_value * 100.0).round() as i16,
                        (humidity_value * 100.0).round() as u16,
                        co2,
                    );
                }
            }
            Err(e) => {
                error!("Failed to read measurements: {:?}", e);
                let _ = self.display.draw_error("Sensor Error");
            }
        }

        Ok(())
    }
}
