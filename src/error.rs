use esp_idf_svc::sys::EspError;
use std::fmt;

/// Application error type.
#[derive(Debug)]
pub enum AppError {
    /// BLE error.
    BleError(String),

    /// Display error.
    DisplayError(String),

    /// I2C error.
    I2cError(String),

    /// Peripherals error.
    PeripheralsError(String),

    /// Sensor error.
    SensorError(String),
}

/// Implement the conversion from `EspError` to `AppError`.
impl From<EspError> for AppError {
    /// Convert an `EspError` to an `AppError`.
    ///
    /// # Parameters
    /// - `error`: The ESP-IDF error.
    ///
    /// # Returns
    /// The application error.
    fn from(error: EspError) -> Self {
        AppError::I2cError(format!("ESP-IDF error: {:?}", error))
    }
}

/// Implement the `Display` trait for `AppError`.
impl fmt::Display for AppError {
    /// Format the error message.
    ///
    /// # Parameters
    /// - `f`: The formatter.
    ///
    /// # Returns
    /// The result of the operation.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::BleError(msg) => write!(f, "BLE error: {}", msg),
            AppError::DisplayError(msg) => write!(f, "Display error: {}", msg),
            AppError::I2cError(msg) => write!(f, "I2C error: {}", msg),
            AppError::PeripheralsError(msg) => write!(f, "Peripherals error: {}", msg),
            AppError::SensorError(msg) => write!(f, "Sensor error: {}", msg),
        }
    }
}

/// Implement the `Error` trait for `AppError`.
impl std::error::Error for AppError {}
