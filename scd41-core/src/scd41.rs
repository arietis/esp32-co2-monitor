/// Measurement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// CO2 concentration in parts per million (ppm).
    pub co2_ppm: u16,

    /// Temperature in degrees Celsius.
    pub temperature_c: f32,

    /// Relative humidity in percent.
    pub humidity_percent: f32,
}

/// Parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Invalid buffer length.
    InvalidLength { expected: usize, actual: usize },

    /// CRC mismatch in chunk.
    CrcMismatch { chunk_index: usize },

    /// Not ready all zeros.
    NotReadyAllZeros,
}

/// Implementation of the `Display` trait for `ParseError`.
impl core::fmt::Display for ParseError {
    /// Format the error message.
    ///
    /// # Arguments
    /// * `f` - The formatter to write the error message to.
    ///
    /// # Returns
    /// * `core::fmt::Result` - The result of the formatting operation.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::InvalidLength { expected, actual } => {
                write!(
                    f,
                    "Invalid buffer length: expected {expected}, got {actual}"
                )
            }
            ParseError::CrcMismatch { chunk_index } => {
                write!(f, "CRC mismatch in chunk {chunk_index}")
            }
            ParseError::NotReadyAllZeros => write!(f, "Sensor returned all zero values"),
        }
    }
}

/// Implementation of the `Error` trait for `ParseError`.
impl std::error::Error for ParseError {}

/// Generate Sensirion CRC-8 (Polynomial: `0x31`, Init: `0xFF`).
///
/// # Arguments
/// * `data` - The data to generate the CRC for.
///
/// # Returns
/// * `u8` - The CRC value.
pub fn crc8_sensirion(data: &[u8]) -> u8 {
    let mut crc = 0xFF;

    for &byte in data {
        crc ^= byte;

        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ 0x31;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

/// The SCD41 returns measurements as 9 bytes:
/// `CO2(2) + CRC(1) + T(2) + CRC(1) + RH(2) + CRC(1)`.
///
/// # Arguments
/// * `buffer` - The buffer containing the measurement data.
///
/// # Returns
/// * `Result<Measurement, ParseError>` - The parsed measurement or an error.
pub fn parse_measurement(buffer: &[u8]) -> Result<Measurement, ParseError> {
    if buffer.len() != 9 {
        return Err(ParseError::InvalidLength {
            expected: 9,
            actual: buffer.len(),
        });
    }

    for (chunk_index, i) in (0..9).step_by(3).enumerate() {
        let expected = buffer[i + 2];

        let actual = crc8_sensirion(&buffer[i..i + 2]);

        if expected != actual {
            return Err(ParseError::CrcMismatch { chunk_index });
        }
    }

    let co2 = u16::from_be_bytes([buffer[0], buffer[1]]);

    let temperature_raw = u16::from_be_bytes([buffer[3], buffer[4]]);

    let humidity_raw = u16::from_be_bytes([buffer[6], buffer[7]]);

    let temperature_c = -45.0 + 175.0 * temperature_raw as f32 / 65535.0;

    let humidity_percent = 100.0 * humidity_raw as f32 / 65535.0;

    if co2 == 0 && temperature_raw == 0 && humidity_raw == 0 {
        return Err(ParseError::NotReadyAllZeros);
    }

    Ok(Measurement {
        co2_ppm: co2,
        temperature_c,
        humidity_percent,
    })
}

/// Tests.
#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(word: u16) -> [u8; 3] {
        let bytes = word.to_be_bytes();
        [bytes[0], bytes[1], crc8_sensirion(&bytes)]
    }

    #[test]
    fn crc_calculation_matches_known_value() {
        // Data: 0xBEEF, CRC: 0x92
        let data = [0xBE, 0xEF];
        assert_eq!(crc8_sensirion(&data), 0x92);
    }

    #[test]
    fn parse_measurement_ok() {
        // Use a non-zero CO2 to avoid NotReadyAllZeros.
        let co2 = chunk(400);
        let t = chunk(0);
        let rh = chunk(0);

        let buffer = [
            co2[0], co2[1], co2[2], t[0], t[1], t[2], rh[0], rh[1], rh[2],
        ];

        let m = parse_measurement(&buffer).unwrap();
        assert_eq!(m.co2_ppm, 400);
        assert!((m.temperature_c - (-45.0)).abs() < 1e-6);
        assert!((m.humidity_percent - 0.0).abs() < 1e-6);
    }

    #[test]
    fn parse_measurement_crc_error() {
        let mut buffer = [0u8; 9];
        buffer[0..3].copy_from_slice(&chunk(400));
        buffer[3..6].copy_from_slice(&chunk(1));
        buffer[6..9].copy_from_slice(&chunk(2));

        buffer[2] ^= 0xFF;
        assert_eq!(
            parse_measurement(&buffer),
            Err(ParseError::CrcMismatch { chunk_index: 0 })
        );
    }

    #[test]
    fn parse_measurement_not_ready_all_zeros() {
        let buffer = [
            0,
            0,
            crc8_sensirion(&[0, 0]),
            0,
            0,
            crc8_sensirion(&[0, 0]),
            0,
            0,
            crc8_sensirion(&[0, 0]),
        ];

        assert_eq!(
            parse_measurement(&buffer),
            Err(ParseError::NotReadyAllZeros)
        );
    }

    #[test]
    fn parse_measurement_invalid_length() {
        assert_eq!(
            parse_measurement(&[]),
            Err(ParseError::InvalidLength {
                expected: 9,
                actual: 0
            })
        );

        assert_eq!(
            parse_measurement(&[0u8; 8]),
            Err(ParseError::InvalidLength {
                expected: 9,
                actual: 8
            })
        );

        assert_eq!(
            parse_measurement(&[0u8; 10]),
            Err(ParseError::InvalidLength {
                expected: 9,
                actual: 10
            })
        );
    }

    #[test]
    fn parse_error_display_messages() {
        let msg = ParseError::InvalidLength {
            expected: 9,
            actual: 8,
        }
        .to_string();
        assert!(msg.contains("expected 9"));
        assert!(msg.contains("got 8"));

        assert_eq!(
            ParseError::CrcMismatch { chunk_index: 2 }.to_string(),
            "CRC mismatch in chunk 2"
        );

        assert_eq!(
            ParseError::NotReadyAllZeros.to_string(),
            "Sensor returned all zero values"
        );
    }
}
