use crate::error::AppError;
use enumset::enum_set;
use esp_idf_svc::{
    bt::{
        ble::{
            gap::{AdvConfiguration, BleGapEvent, EspBleGap},
            gatt::{
                server::{ConnectionId, EspGatts, GattsEvent, TransferId},
                AutoResponse, GattCharacteristic, GattDescriptor, GattId, GattInterface,
                GattResponse, GattServiceId, GattStatus, Handle, Permission, Property,
            },
        },
        BdAddr, Ble, BtDriver, BtStatus, BtUuid,
    },
    hal::modem::Modem,
    nvs::{EspNvsPartition, NvsDefault},
    sys::{EspError, ESP_FAIL},
};
use log::{info, warn};
use std::sync::{Arc, Mutex};

/// CO2 characteristic UUID.
pub const CO2_CHAR_UUID: u128 = 0x00002b8c00001000800000805f9b34fb;

/// Humidity characteristic UUID.
pub const HUMIDITY_CHAR_UUID: u128 = 0x00002a6f00001000800000805f9b34fb;

/// Service UUID.
pub const SERVICE_UUID: u128 = 0xc892f08b050249a68c52b959aa997e54;

/// Temperature characteristic UUID.
pub const TEMPERATURE_CHAR_UUID: u128 = 0x00002a6e00001000800000805f9b34fb;

/// Application ID.
const APP_ID: u16 = 0;

/// Device name.
const DEVICE_NAME: &str = "ESP32-CO2";

/// Maximum number of connections.
const MAX_CONNECTIONS: usize = 2;

/// Connection interface.
#[derive(Debug, Clone)]
struct Connection {
    /// Peer address.
    peer: BdAddr,

    /// Connection ID.
    conn_id: Handle,

    /// Subscribed status.
    subscribed: bool,

    /// MTU.
    mtu: Option<u16>,
}

/// State interface.
#[derive(Default)]
struct State {
    /// GATT interface.
    gatt_if: Option<GattInterface>,

    /// Service handle.
    service_handle: Option<Handle>,

    /// Temperature handle.
    temp_handle: Option<Handle>,

    /// Humidity handle.
    humid_handle: Option<Handle>,

    /// CO2 handle.
    co2_handle: Option<Handle>,

    /// Temperature CCCD handle.
    temp_cccd_handle: Option<Handle>,

    /// Humidity CCCD handle.
    humid_cccd_handle: Option<Handle>,

    /// CO2 CCCD handle.
    co2_cccd_handle: Option<Handle>,

    /// Connections.
    connections: heapless::Vec<Connection, MAX_CONNECTIONS>,

    /// GATT response.
    response: GattResponse,

    /// Latest temperature.
    latest_temperature: i16,

    /// Latest humidity.
    latest_humidity: u16,

    /// Latest CO2.
    latest_co2: u16,
}

/// BLE server interface.
#[derive(Clone)]
pub struct BleServer {
    /// GATT server.
    gatts: Arc<EspGatts<'static, Ble, Arc<BtDriver<'static, Ble>>>>,

    /// GAP service.
    gap: Arc<EspBleGap<'static, Ble, Arc<BtDriver<'static, Ble>>>>,

    /// State.
    state: Arc<Mutex<State>>,
}

/// BLE server implementation.
impl BleServer {
    /// Create a new BLE server.
    ///
    /// # Arguments
    ///
    /// * `modem` - The modem to use for Bluetooth communication.
    /// * `nvs` - The NVS partition to use for storing data.
    ///
    /// # Returns
    ///
    /// A new instance of `BleServer` or an error if initialization fails.
    pub fn new(
        modem: Modem<'static>,
        nvs: Option<EspNvsPartition<NvsDefault>>,
    ) -> Result<Self, AppError> {
        info!("Initializing BLE server");

        let bt_driver =
            Arc::new(BtDriver::new(modem, nvs.clone()).map_err(|e| {
                AppError::BleError(format!("Failed to initialize Bluetooth: {:?}", e))
            })?);
        info!("Bluetooth driver initialized");

        let gap = Arc::new(EspBleGap::new(bt_driver.clone()).map_err(|e| {
            AppError::BleError(format!("Failed to initialize GAP service: {:?}", e))
        })?);
        info!("BLE Gap initialized");

        let gatts = Arc::new(EspGatts::new(bt_driver.clone()).map_err(|e| {
            AppError::BleError(format!("Failed to initialize GATT server: {:?}", e))
        })?);
        info!("BLE Gatts initialized");

        let server = Self {
            gatts,
            gap,
            state: Arc::new(Mutex::new(Default::default())),
        };

        // GAP events
        let gap_server = server.clone();
        server
            .gap
            .subscribe(move |event| {
                gap_server.check_esp_status(gap_server.on_gap_event(event));
            })
            .map_err(|e| {
                AppError::BleError(format!("Failed to subscribe to GAP events: {:?}", e))
            })?;

        // GATTS events
        let gatts_server = server.clone();
        server
            .gatts
            .subscribe(move |(gatt_if, event)| {
                gatts_server.check_esp_status(gatts_server.on_gatts_event(gatt_if, event))
            })
            .map_err(|e| {
                AppError::BleError(format!("Failed to subscribe to GATT events: {:?}", e))
            })?;
        info!("BLE Gap and Gatts subscriptions initialized");

        server
            .gatts
            .register_app(APP_ID)
            .map_err(|e| AppError::BleError(format!("Failed to register app: {:?}", e)))?;
        info!("Gatts BTP app registered");

        Ok(server)
    }

    /// Check the GATT status and return an error if it is not Ok.
    ///
    /// # Arguments
    /// * `status` - The GATT status to check.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    fn check_gatt_status(&self, status: GattStatus) -> Result<(), EspError> {
        if !matches!(status, GattStatus::Ok) {
            warn!("Got GATT status: {:?}", status);
            Err(EspError::from_infallible::<ESP_FAIL>())
        } else {
            Ok(())
        }
    }

    /// Check the BT status and return an error if it is not Ok.
    ///
    /// # Arguments
    /// * `status` - The BT status to check.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    fn check_bt_status(&self, status: BtStatus) -> Result<(), EspError> {
        if !matches!(status, BtStatus::Success) {
            warn!("Got BT status: {:?}", status);
            Err(EspError::from_infallible::<ESP_FAIL>())
        } else {
            Ok(())
        }
    }

    /// Check the ESP status and return an error if it is not Ok.
    ///
    /// # Arguments
    /// * `status` - The ESP status to check.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    fn check_esp_status(&self, status: Result<(), EspError>) {
        if let Err(e) = status {
            warn!("Got ESP status: {:?}", e);
        }
    }

    /// Create the service once the app is registered.
    ///
    /// # Arguments
    /// * `gatt_if` - The GATT interface to use.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    fn create_service(&self, gatt_if: GattInterface) -> Result<(), EspError> {
        {
            let mut state = self.state.lock().unwrap();
            state.gatt_if = Some(gatt_if);
        }

        self.gap.set_device_name(DEVICE_NAME)?;
        self.gap.set_adv_conf(&AdvConfiguration {
            include_name: true,
            include_txpower: true,
            flag: 2,
            service_uuid: Some(BtUuid::uuid128(SERVICE_UUID)),
            ..Default::default()
        })?;

        self.gatts.create_service(
            gatt_if,
            &GattServiceId {
                id: GattId {
                    uuid: BtUuid::uuid128(SERVICE_UUID),
                    inst_id: 0,
                },
                is_primary: true,
            },
            16, // enough handles for 3 chars + CCCD
        )?;

        Ok(())
    }

    /// Configure and start the service once it is created.
    ///
    /// # Arguments
    /// * `service_handle` - The handle of the service to configure and start.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    fn configure_and_start_service(&self, service_handle: Handle) -> Result<(), EspError> {
        {
            let mut state = self.state.lock().unwrap();
            state.service_handle = Some(service_handle);
            state.temp_handle = None;
            state.humid_handle = None;
            state.co2_handle = None;
            state.temp_cccd_handle = None;
            state.humid_cccd_handle = None;
            state.co2_cccd_handle = None;
        }

        self.gatts.start_service(service_handle)?;
        self.add_characteristics(service_handle)?;

        Ok(())
    }

    /// Add characteristics to the service.
    ///
    /// # Arguments
    /// * `service_handle` - The handle of the service to add characteristics to.
    ///
    /// # Returns
    ///
    /// A result indicating success or failure.
    fn add_characteristics(&self, service_handle: Handle) -> Result<(), EspError> {
        self.gatts.add_characteristic(
            service_handle,
            &GattCharacteristic {
                uuid: BtUuid::uuid128(TEMPERATURE_CHAR_UUID),
                permissions: enum_set!(Permission::Read | Permission::Write),
                properties: enum_set!(Property::Read | Property::Notify),
                max_len: 6,
                auto_rsp: AutoResponse::ByApp,
            },
            &[],
        )?;
        self.gatts.add_characteristic(
            service_handle,
            &GattCharacteristic {
                uuid: BtUuid::uuid128(HUMIDITY_CHAR_UUID),
                permissions: enum_set!(Permission::Read | Permission::Write),
                properties: enum_set!(Property::Read | Property::Notify),
                max_len: 6,
                auto_rsp: AutoResponse::ByApp,
            },
            &[],
        )?;
        self.gatts.add_characteristic(
            service_handle,
            &GattCharacteristic {
                uuid: BtUuid::uuid128(CO2_CHAR_UUID),
                permissions: enum_set!(Permission::Read | Permission::Write),
                properties: enum_set!(Property::Read | Property::Notify),
                max_len: 6,
                auto_rsp: AutoResponse::ByApp,
            },
            &[],
        )?;

        Ok(())
    }

    /// Handle GAP events.
    ///
    /// # Arguments
    /// * `event` - The GAP event to handle.
    ///
    /// # Returns
    ///
    /// * `Result<(), EspError>` - The result of handling the event.
    fn on_gap_event(&self, event: BleGapEvent) -> Result<(), EspError> {
        info!("Got GAP event: {event:?}");

        if let BleGapEvent::AdvertisingConfigured(status) = event {
            self.check_bt_status(status)?;
            info!("Advertising configured, starting advertising...");
            self.gap.start_advertising()?;
        }

        Ok(())
    }

    /// Handle GATT server events.
    ///
    /// # Arguments
    /// * `gatt_if` - The GATT interface.
    /// * `event` - The GATT event to handle.
    ///
    /// # Returns
    ///
    /// * `Result<(), EspError>` - The result of handling the event.
    fn on_gatts_event(&self, gatt_if: GattInterface, event: GattsEvent) -> Result<(), EspError> {
        info!("Got GATTS event: {event:?}");

        match event {
            GattsEvent::ServiceRegistered { status, app_id } => {
                self.check_gatt_status(status)?;
                if APP_ID == app_id {
                    self.create_service(gatt_if)?;
                }
            }
            GattsEvent::ServiceCreated {
                status,
                service_handle,
                ..
            } => {
                self.check_gatt_status(status)?;
                self.configure_and_start_service(service_handle)?;
            }
            GattsEvent::ServiceStarted {
                status,
                service_handle,
            } => {
                self.check_gatt_status(status)?;
                info!("Service started, handle = {}", service_handle);
            }
            GattsEvent::CharacteristicAdded {
                status,
                attr_handle,
                service_handle,
                char_uuid,
            } => {
                self.check_gatt_status(status)?;
                self.register_characteristic(service_handle, attr_handle, char_uuid)?;
            }
            GattsEvent::DescriptorAdded {
                status,
                attr_handle,
                service_handle,
                descr_uuid,
            } => {
                self.check_gatt_status(status)?;
                self.register_descriptor(service_handle, attr_handle, descr_uuid)?;
            }
            GattsEvent::Mtu { conn_id, mtu } => {
                let mut state = self.state.lock().unwrap();
                if let Some(conn) = state.connections.iter_mut().find(|c| c.conn_id == conn_id) {
                    conn.mtu = Some(mtu);
                }
                info!("Connection {} negotiated an MTU of {}", conn_id, mtu);
            }
            GattsEvent::PeerConnected { conn_id, addr, .. } => {
                info!("Peer connected: conn_id = {}, addr = {}", conn_id, addr);
                let mut state = self.state.lock().unwrap();
                if state.connections.len() < MAX_CONNECTIONS {
                    state
                        .connections
                        .push(Connection {
                            peer: addr,
                            conn_id,
                            subscribed: false,
                            mtu: None,
                        })
                        .ok();
                }
            }
            GattsEvent::PeerDisconnected { conn_id, addr, .. } => {
                info!("Peer disconnected: conn_id = {}, addr = {}", conn_id, addr);
                let mut state = self.state.lock().unwrap();
                if let Some(pos) = state.connections.iter().position(|c| c.conn_id == conn_id) {
                    state.connections.remove(pos);
                }
                drop(state);
                self.gap.start_advertising()?;
            }
            GattsEvent::Write {
                conn_id,
                trans_id,
                addr,
                handle,
                offset,
                need_rsp,
                is_prep,
                value,
            } => {
                let handled = self.handle_write(
                    gatt_if, conn_id, trans_id, addr, handle, offset, need_rsp, is_prep, value,
                )?;

                if handled && need_rsp {
                    self.send_write_response(
                        gatt_if, conn_id, trans_id, handle, offset, need_rsp, is_prep, value,
                    )?;
                }
            }
            GattsEvent::Confirm { status, handle, .. } => {
                if status == GattStatus::Ok {
                    info!("Indication/notification confirmed for handle {}", handle);
                } else {
                    warn!(
                        "Indication/notification failed for handle {}: {:?}",
                        handle, status
                    );
                }
            }
            GattsEvent::Read {
                conn_id,
                trans_id,
                handle,
                offset,
                need_rsp,
                ..
            } => {
                if need_rsp {
                    let mut response = GattResponse::new();

                    let data = {
                        let state = self.state.lock().unwrap();
                        if Some(handle) == state.temp_handle {
                            Some(state.latest_temperature.to_le_bytes().to_vec())
                        } else if Some(handle) == state.humid_handle {
                            Some(state.latest_humidity.to_le_bytes().to_vec())
                        } else if Some(handle) == state.co2_handle {
                            Some(state.latest_co2.to_le_bytes().to_vec())
                        } else {
                            None
                        }
                    };

                    if let Some(value) = data {
                        response
                            .attr_handle(handle)
                            .offset(offset)
                            .auth_req(0)
                            .value(&value)
                            .map_err(|_| EspError::from_infallible::<ESP_FAIL>())?;

                        self.gatts.send_response(
                            gatt_if,
                            conn_id,
                            trans_id,
                            GattStatus::Ok,
                            Some(&response),
                        )?;
                    } else {
                        self.gatts.send_response(
                            gatt_if,
                            conn_id,
                            trans_id,
                            GattStatus::NotFound,
                            None,
                        )?;
                    }
                }
            }
            _ => (),
        }

        Ok(())
    }

    /// Register a characteristic.
    ///
    /// # Arguments
    /// * `service_handle` - The service handle.
    /// * `attr_handle` - The attribute handle.
    /// * `char_uuid` - The characteristic UUID.
    ///
    /// # Returns
    ///
    /// * `Result<(), EspError>` - The result of registering the characteristic.
    fn register_characteristic(
        &self,
        service_handle: Handle,
        attr_handle: Handle,
        char_uuid: BtUuid,
    ) -> Result<(), EspError> {
        let mut state = self.state.lock().unwrap();

        if state.service_handle != Some(service_handle) {
            return Ok(());
        }

        if char_uuid == BtUuid::uuid128(TEMPERATURE_CHAR_UUID) {
            state.temp_handle = Some(attr_handle);
            self.gatts.add_descriptor(
                service_handle,
                &GattDescriptor {
                    uuid: BtUuid::uuid16(0x2902), // CCCD
                    permissions: enum_set!(Permission::Read | Permission::Write),
                },
            )?;
        } else if char_uuid == BtUuid::uuid128(HUMIDITY_CHAR_UUID) {
            state.humid_handle = Some(attr_handle);
            self.gatts.add_descriptor(
                service_handle,
                &GattDescriptor {
                    uuid: BtUuid::uuid16(0x2902),
                    permissions: enum_set!(Permission::Read | Permission::Write),
                },
            )?;
        } else if char_uuid == BtUuid::uuid128(CO2_CHAR_UUID) {
            state.co2_handle = Some(attr_handle);
            self.gatts.add_descriptor(
                service_handle,
                &GattDescriptor {
                    uuid: BtUuid::uuid16(0x2902),
                    permissions: enum_set!(Permission::Read | Permission::Write),
                },
            )?;
        }

        Ok(())
    }

    /// Register a descriptor.
    ///
    /// # Arguments
    /// * `service_handle` - The service handle.
    /// * `attr_handle` - The attribute handle.
    /// * `descr_uuid` - The descriptor UUID.
    ///
    /// # Returns
    ///
    /// * `Result<(), EspError>` - The result of registering the descriptor.
    fn register_descriptor(
        &self,
        service_handle: Handle,
        attr_handle: Handle,
        descr_uuid: BtUuid,
    ) -> Result<(), EspError> {
        let mut state = self.state.lock().unwrap();

        if state.service_handle != Some(service_handle) {
            return Ok(());
        }

        if descr_uuid == BtUuid::uuid16(0x2902) {
            if state.temp_handle.is_some() && state.temp_cccd_handle.is_none() {
                state.temp_cccd_handle = Some(attr_handle);
            } else if state.humid_handle.is_some() && state.humid_cccd_handle.is_none() {
                state.humid_cccd_handle = Some(attr_handle);
            } else if state.co2_handle.is_some() && state.co2_cccd_handle.is_none() {
                state.co2_cccd_handle = Some(attr_handle);
            }
        }

        Ok(())
    }

    /// Handle a write request.
    ///
    /// # Arguments
    /// * `gatt_if` - The GATT interface.
    /// * `conn_id` - The connection ID.
    /// * `trans_id` - The transfer ID.
    /// * `addr` - The Bluetooth address.
    /// * `handle` - The attribute handle.
    /// * `offset` - The offset.
    /// * `need_rsp` - Whether a response is needed.
    /// * `is_prep` - Whether the write is a prepare write.
    /// * `value` - The value to write.
    ///
    /// # Returns
    ///
    /// * `Result<bool, EspError>` - The result of handling the write request.
    fn handle_write(
        &self,
        _gatt_if: GattInterface,
        conn_id: ConnectionId,
        _trans_id: TransferId,
        addr: BdAddr,
        handle: Handle,
        _offset: u16,
        _need_rsp: bool,
        _is_prep: bool,
        value: &[u8],
    ) -> Result<bool, EspError> {
        let mut state = self.state.lock().unwrap();

        let handled = if Some(handle) == state.temp_cccd_handle
            || Some(handle) == state.humid_cccd_handle
            || Some(handle) == state.co2_cccd_handle
        {
            self.set_subscription(&mut state, conn_id, addr, value)?;
            true
        } else {
            false
        };

        Ok(handled)
    }

    /// Send a write response.
    ///
    /// # Arguments
    /// * `gatt_if` - The GATT interface.
    /// * `conn_id` - The connection ID.
    /// * `trans_id` - The transfer ID.
    /// * `handle` - The attribute handle.
    /// * `offset` - The offset.
    /// * `need_rsp` - Whether a response is needed.
    /// * `is_prep` - Whether the write is a prepare write.
    /// * `value` - The value to write.
    ///
    /// # Returns
    ///
    /// * `Result<(), EspError>` - The result of sending the write response.
    fn send_write_response(
        &self,
        gatt_if: GattInterface,
        conn_id: ConnectionId,
        trans_id: TransferId,
        handle: Handle,
        offset: u16,
        need_rsp: bool,
        is_prep: bool,
        value: &[u8],
    ) -> Result<(), EspError> {
        if !need_rsp {
            return Ok(());
        }

        if is_prep {
            let mut state = self.state.lock().unwrap();
            state
                .response
                .attr_handle(handle)
                .auth_req(0)
                .offset(offset)
                .value(value)
                .map_err(|_| EspError::from_infallible::<ESP_FAIL>())?;

            self.gatts.send_response(
                gatt_if,
                conn_id,
                trans_id,
                GattStatus::Ok,
                Some(&state.response),
            )?;
        } else {
            self.gatts
                .send_response(gatt_if, conn_id, trans_id, GattStatus::Ok, None)?;
        }

        Ok(())
    }

    /// Set subscription status.
    ///
    /// # Arguments
    /// * `state` - The state.
    /// * `conn_id` - The connection ID.
    /// * `addr` - The address.
    /// * `value` - The value.
    ///
    /// # Returns
    ///
    /// * `Result<(), EspError>` - The result of setting the subscription status.
    fn set_subscription(
        &self,
        state: &mut State,
        conn_id: ConnectionId,
        addr: BdAddr,
        value: &[u8],
    ) -> Result<(), EspError> {
        if value.len() == 2 {
            let flags = u16::from_le_bytes([value[0], value[1]]);
            let enable_notify = flags & 0x0001 != 0; // bit 0 = notifications

            if let Some(conn) = state.connections.iter_mut().find(|c| c.conn_id == conn_id) {
                conn.subscribed = enable_notify;
            }

            if enable_notify {
                info!("Notifications enabled by {}", addr);
            } else {
                info!("Notifications disabled by {}", addr);
            }
        }

        Ok(())
    }

    /// Update characteristic values and notify subscribers.
    ///
    /// # Arguments
    /// * `temperature` - The temperature.
    /// * `humidity` - The humidity.
    /// * `co2` - The CO2.
    ///
    /// # Returns
    ///
    /// * `Result<(), EspError>` - The result of updating the values.
    pub fn update_values(&self, temperature: i16, humidity: u16, co2: u16) {
        let mut state = self.state.lock().unwrap();
        state.latest_temperature = temperature;
        state.latest_humidity = humidity;
        state.latest_co2 = co2;

        let Some(gatt_if) = state.gatt_if else {
            return;
        };

        if let Some(handle) = state.temp_handle {
            let temp_bytes = temperature.to_le_bytes();
            if let Err(e) = self.gatts.set_attr(handle, &temp_bytes) {
                warn!("Failed to set temperature attribute: {:?}", e);
            }

            for conn in state.connections.iter() {
                if conn.subscribed {
                    if let Err(e) = self
                        .gatts
                        .notify(gatt_if, conn.conn_id, handle, &temp_bytes)
                    {
                        warn!("Failed to send temperature notification: {:?}", e);
                    }
                }
            }
        }

        if let Some(handle) = state.humid_handle {
            let humid_bytes = humidity.to_le_bytes();
            if let Err(e) = self.gatts.set_attr(handle, &humid_bytes) {
                warn!("Failed to set humidity attribute: {:?}", e);
            }

            for conn in state.connections.iter() {
                if conn.subscribed {
                    if let Err(e) = self
                        .gatts
                        .notify(gatt_if, conn.conn_id, handle, &humid_bytes)
                    {
                        warn!("Failed to send humidity notification: {:?}", e);
                    }
                }
            }
        }

        if let Some(handle) = state.co2_handle {
            let co2_bytes = co2.to_le_bytes();
            if let Err(e) = self.gatts.set_attr(handle, &co2_bytes) {
                warn!("Failed to set CO2 attribute: {:?}", e);
            }

            for conn in state.connections.iter() {
                if conn.subscribed {
                    if let Err(e) = self.gatts.notify(gatt_if, conn.conn_id, handle, &co2_bytes) {
                        warn!("Failed to send CO2 notification: {:?}", e);
                    }
                }
            }
        }
    }
}
