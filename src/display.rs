use crate::error::AppError;
use crate::font::FONT_6X8;
use esp_idf_svc::hal::i2c::I2cDriver;
use std::cell::RefCell;
use std::rc::Rc;

/// Display width.
const DISPLAY_WIDTH: u8 = 128;

/// Initialization sequence.
const INIT_SEQUENCE: &[u8] = &[
  0xae, // display off
  0xd5, // set display clock
  0x80, //
  0xa8, // set multiplex ratio
  0x3f, //
  0xd3, // set display offset
  0x00, // no offset
  0x40, // set start line
  0x8d, // charge pump
  0x14, // enable charge pump
  0x20, // memory mode
  0x00, // horizontal addressing
  0xa1, // segment remap
  0xc8, // com scan direction
  0xda, // set com pins
  0x12, //
  0x81, // set contrast
  0xcf, //
  0xd9, // set precharge
  0xf1, //
  0xdb, // set vcom detect
  0x40, //
  0xa4, // display all on resume
  0xa6, // normal display
  0xaf, // display on
];

/// SSD1306 I2C address.
const SSD1306_ADDRESS: u8 = 0x3d;

/// SSD1306 display interface.
pub struct Ssd1306Display<'a> {
  /// The I2C driver.
  i2c: Rc<RefCell<I2cDriver<'a>>>,
}

/// The SSD1306 display implementation.
impl<'a> Ssd1306Display<'a> {
  /// Create a new SSD1306 display.
  ///
  /// # Parameters
  /// - `i2c`: The I2C driver.
  ///
  /// # Returns
  /// The SSD1306 display.
  pub fn new(i2c: Rc<RefCell<I2cDriver<'a>>>) -> Result<Self, AppError> {
    Ok(Self { i2c })
  }

  /// Initialize the display.
  ///
  /// # Returns
  /// The result of the operation.
  pub fn init(&mut self) -> Result<(), AppError> {
    let mut i2c = self.i2c.borrow_mut();

    for &cmd in INIT_SEQUENCE {
      self.write_command(&mut i2c, cmd)?;
    }

    Ok(())
  }

  /// Clear the display.
  ///
  /// # Returns
  /// The result of the operation.
  pub fn clear(&mut self) -> Result<(), AppError> {
    let mut i2c = self.i2c.borrow_mut();

    for page in 0..8 {
      self.set_cursor(&mut i2c, 0, page)?;

      for _ in 0..DISPLAY_WIDTH {
        self.write_data(&mut i2c, 0)?;
      }
    }

    Ok(())
  }

  /// Draw measurements on the display.
  ///
  /// # Parameters
  /// - `co2`: The CO2 measurement.
  /// - `temperature`: The temperature measurement.
  /// - `humidity`: The humidity measurement.
  ///
  /// # Returns
  /// The result of the operation.
  pub fn draw_measurements(
    &mut self,
    co2: u16,
    temperature: f32,
    humidity: f32
  ) -> Result<(), AppError> {
    self.clear()?;

    // Format measurements
    let co2_str = format!("CO2: {} ppm", co2);

    let temp_str = format!("Temp: {:.1} C", temperature);

    let hum_str = format!("Hum: {:.1} %", humidity);

    // Draw each line
    let mut i2c = self.i2c.borrow_mut();
    self.draw_text_internal(&mut i2c, &co2_str, 0)?;
    self.draw_text_internal(&mut i2c, &temp_str, 2)?;
    self.draw_text_internal(&mut i2c, &hum_str, 4)?;

    Ok(())
  }

  /// Draw an error message on the display.
  ///
  /// # Parameters
  /// - `error`: The error message.
  ///
  /// # Returns
  /// The result of the operation.
  pub fn draw_error(&mut self, error: &str) -> Result<(), AppError> {
    self.clear()?;

    let mut i2c = self.i2c.borrow_mut();
    self.draw_text_internal(&mut i2c, error, 2)
  }

  /// Write a command to the display.
  ///
  /// # Parameters
  /// - `i2c`: The I2C driver.
  /// - `cmd`: The command.
  ///
  /// # Returns
  /// The result of the operation.
  fn write_command(&self, i2c: &mut I2cDriver<'a>, cmd: u8) -> Result<(), AppError> {
    i2c.write(SSD1306_ADDRESS, &[0x00, cmd], 100)
      .map_err(|e| AppError::DisplayError(format!(
        "Failed to write command 0x{:02x} to display at address 0x{:02x}: {:?}",
        cmd, SSD1306_ADDRESS, e
      )))
  }

  /// Write data to the display.
  ///
  /// # Parameters
  /// - `i2c`: The I2C driver.
  /// - `data`: The data.
  ///
  /// # Returns
  /// The result of the operation.
  fn write_data(&self, i2c: &mut I2cDriver<'a>, data: u8) -> Result<(), AppError> {
    i2c.write(SSD1306_ADDRESS, &[0x40, data], 100)
      .map_err(|e| AppError::DisplayError(format!(
        "Failed to write data 0x{:02x} to display at address 0x{:02x}: {:?}",
        data, SSD1306_ADDRESS, e
      )))
  }

  /// Set the cursor position.
  ///
  /// # Parameters
  /// - `i2c`: The I2C driver.
  /// - `x`: The X position.
  /// - `page`: The page.
  ///
  /// # Returns
  /// The result of the operation.
  fn set_cursor(&self, i2c: &mut I2cDriver<'a>, x: u8, page: u8) -> Result<(), AppError> {
    self.write_command(i2c, 0xb0 | page)?; // Set page
    self.write_command(i2c, x & 0xf)?; // Set lower column start address
    self.write_command(i2c, 0x10 | (x >> 4))?; // Set higher column start address

    Ok(())
  }

  /// Draw text on the display.
  ///
  /// # Parameters
  /// - `i2c`: The I2C driver.
  /// - `text`: The text.
  /// - `page`: The page.
  ///
  /// # Returns
  /// The result of the operation.
  fn draw_text_internal(
    &self,
    i2c: &mut I2cDriver<'a>,
    text: &str,
    page: u8
  ) -> Result<(), AppError> {
    self.set_cursor(i2c, 0, page)?;

    for c in text.chars() {
      let char_index = c as usize;

      if char_index >= 32 && char_index < (32 + FONT_6X8.len()) {
        let index = char_index - 32;

        let char_data = &FONT_6X8[index];

        for &byte in char_data {
          self.write_data(i2c, byte)?;
        }
      }
    }

    Ok(())
  }
}
