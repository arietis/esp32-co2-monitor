# ESP32 CO2 Monitor

A Rust-based air quality monitoring system using ESP32 microcontroller, SCD41 CO2 sensor, and SSD1306 OLED display.

## Features

- Measures CO2, temperature, and humidity using Sensirion SCD41 sensor
- Displays readings on a SSD1306 OLED display
- Written in Rust using esp-idf framework
- Periodic measurements with configurable interval
- Error handling and display

## Hardware Requirements

- ESP32 development board
- Sensirion SCD41 CO2 sensor
- SSD1306 OLED display (128x64)
- I2C connections:
  - SDA: GPIO8
  - SCL: GPIO9

## Building and Flashing

1. Install Rust and ESP-IDF toolchain
2. Clone the repository:
   ```bash
   git clone https://github.com/arietis/esp32-co2-monitor.git
   cd esp32-co2-monitor
   ```
3. Build the project:
   ```bash
   cargo build
   ```
4. Flash to ESP32:
   ```bash
   cargo run
   ```

## Configuration

The default measurement interval is 5 seconds. You can modify this in `src/main.rs`:

```rust
const MEASUREMENT_INTERVAL_MS: u32 = 5000;
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.
