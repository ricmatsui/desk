use super::input::Input;
use super::pixels::Pixels;
use super::thinkink::ThinkInk;
use super::{ApiClient, Context, TogglError};
use ::core::str::FromStr;
use raylib::prelude::*;
use serialport::{available_ports, SerialPort, SerialPortInfo, SerialPortType};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::{env, io, str};

pub struct MacroPad {
    serial_port: Option<Box<dyn SerialPort>>,
    input_buffer: Vec<u8>,
    output_buffer: Vec<u8>,
    api_client: Arc<dyn ApiClient>,
    input: Rc<RefCell<Input>>,
    pixels: Rc<RefCell<Pixels>>,
    thinkink: Rc<RefCell<ThinkInk>>,
    last_update: Option<f64>,
    current_start: Option<chrono::DateTime<chrono::Utc>>,
}

impl MacroPad {
    pub fn new(
        api_client: Arc<dyn ApiClient>,
        input: Rc<RefCell<Input>>,
        pixels: Rc<RefCell<Pixels>>,
        thinkink: Rc<RefCell<ThinkInk>>,
    ) -> Self {
        Self {
            serial_port: None,
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            api_client,
            input,
            pixels,
            thinkink,
            last_update: None,
            current_start: None,
        }
    }

    pub fn open_serial(&mut self) {
        if self.serial_port.is_some() {
            return;
        }

        let matching_port_infos: Vec<SerialPortInfo> = available_ports()
            .unwrap()
            .into_iter()
            .filter(|port_info| match port_info.port_type {
                SerialPortType::UsbPort(ref usb_info) => {
                    return usb_info.vid == 0x239A && usb_info.pid == 0x8108
                }
                _ => false,
            })
            .collect();

        for port_info in matching_port_infos {
            let mut port = match serialport::new(port_info.port_name.to_string(), 19200).open() {
                Ok(port) => port,
                Err(_) => continue,
            };

            let mut buffer = [0; 2];

            let read_count = match port.read(&mut buffer) {
                Ok(count) => count,
                Err(_) => continue,
            };

            if &buffer[0..read_count] != b"h\n" {
                continue;
            }

            self.serial_port = Some(port);
            self.input_buffer.clear();
            self.output_buffer.clear();
            log::debug!("= connected");
            break;
        }
    }

    pub fn update(&mut self, context: &Context, rl: &RaylibHandle) {
        self.update_buffers();
        self.process_input();

        if context.input.borrow().is_key_pressed(KeyboardKey::KEY_ONE) {
            self.api_client.send_wake_on_lan();
        }

        if self.last_update.is_none() || rl.get_time() - self.last_update.unwrap() > 60.0 {
            match self.get_current_time_entry() {
                Ok(current_time_entry) => {
                    if current_time_entry.is_null() {
                        if self.current_start.is_some() {
                            self.thinkink.borrow_mut().send_message(json::object! {
                                kind: "stopAnimation",
                            });
                        }

                        self.current_start = None;
                    } else {
                        if self.current_start.is_none() {
                            self.thinkink.borrow_mut().send_message(json::object! {
                                kind: "startAnimation",
                            });
                        }

                        self.current_start = Some(chrono::DateTime::from(
                            chrono::DateTime::parse_from_rfc3339(
                                current_time_entry["start"].as_str().unwrap(),
                            )
                            .unwrap(),
                        ));
                    }
                }
                Err(_) => {
                    self.current_start = None;
                }
            }

            self.last_update = Some(rl.get_time());
        }

        let mut pixels = self.pixels.borrow_mut();

        match self.current_start {
            Some(current_start) => {
                let duration = chrono::Utc::now() - current_start;

                let num_seconds = duration.num_milliseconds() as f32 / 1000.0;

                let mut components = vec![[0; 3]; pixels.len()];

                for c in 0..3 {
                    let offset_seconds = (c * pixels.len()) as f32 * 60.0;
                    let display_seconds = (num_seconds - offset_seconds).max(0.0);
                    let lit_count = display_seconds / 60.0;

                    let active_count = (pixels.len() as i32 - lit_count as i32)
                        .clamp(0, pixels.len() as i32 - 1)
                        as usize;

                    let active_index = active_count as f32
                        - display_seconds % 60.0 / 60.0 * (active_count + 1) as f32;

                    if display_seconds > 0.0 {
                        for i in pixels.len() - 1 - active_count..pixels.len() {
                            let value =
                                1.0 - (lit_count + active_index - i as f32).abs().clamp(0.0, 1.0);

                            components[i][c] = (value * 255.0) as u8;
                        }

                        for i in 0..pixels.len() - active_count {
                            components[i][c] = 255;
                        }
                    }
                }

                for i in 0..pixels.len() {
                    pixels.set_pixel(
                        i,
                        Color::new(components[i][0], components[i][1], components[i][2], 255)
                            .fade(0.13),
                    );
                }

                if !pixels.enabled() {
                    pixels.set_enabled(true);
                }
            }
            None => {
                if pixels.enabled() {
                    pixels.set_enabled(false);
                }
            }
        }
    }

    fn update_buffers(&mut self) {
        let port = match self.serial_port.as_mut() {
            Some(port) => port,
            None => return,
        };

        let mut disconnected = false;

        let mut input = [0; 256];
        match port.read(&mut input) {
            Ok(read_count) => {
                self.input_buffer.extend_from_slice(&input[0..read_count]);
            }
            Err(e) => {
                if e.kind() != io::ErrorKind::TimedOut {
                    log::debug!("= error reading");
                    disconnected = true;
                }
            }
        }

        let bytes_to_write = match port.bytes_to_write() {
            Ok(bytes) => bytes,
            Err(_) => {
                log::debug!("= error checking bytes to write");
                disconnected = true;
                0
            }
        };

        while self.output_buffer.len() > 0 && bytes_to_write < 256 && !disconnected {
            let output: Vec<u8> = self.output_buffer.iter().take(256).copied().collect();

            match port.write(&output) {
                Ok(write_count) => {
                    self.output_buffer.drain(0..write_count);
                }
                Err(e) => {
                    if e.kind() != io::ErrorKind::TimedOut {
                        log::debug!("= error writing");
                        disconnected = true;
                    }

                    break;
                }
            }
        }

        if disconnected {
            self.serial_port = None;
            log::debug!("= disconnected");
        }
    }

    fn process_input(&mut self) {
        while let Some(new_line_index) = self.input_buffer.iter().position(|&c| c == '\n' as u8) {
            let message_buffer: Vec<u8> = self.input_buffer.drain(0..=new_line_index).collect();

            let message_string = match str::from_utf8(&message_buffer) {
                Ok(message_string) => message_string,
                Err(_) => {
                    log::warn!("= invalid utf-8 message: {:?}", message_buffer);
                    continue;
                }
            };

            if message_string == "h\n" {
                continue;
            }

            // Process message starting with p and parse the rest of the string as an integer
            if message_string.starts_with("p") {
                let value = message_string[1..message_string.len() - 1]
                    .parse::<u32>()
                    .unwrap();
                self.input.borrow_mut().set_z_axis(value as f32 / 1023.0);
                continue;
            }

            if message_string.starts_with("x") {
                let value = message_string[1..message_string.len() - 1]
                    .parse::<i32>()
                    .unwrap();
                self.input.borrow_mut().set_x_axis(value as f32 / 32768.0);
                continue;
            }

            if message_string.starts_with("y") {
                let value = message_string[1..message_string.len() - 1]
                    .parse::<i32>()
                    .unwrap();
                self.input.borrow_mut().set_y_axis(value as f32 / 32768.0);
                continue;
            }

            let message = match json::parse(message_string) {
                Ok(message) => message,
                Err(_) => {
                    log::warn!("= invalid json message: {}", message_string);
                    continue;
                }
            };

            self.process_message(&message);
        }
    }

    fn process_message(&mut self, message: &json::JsonValue) {
        log::debug!("<- {}", message.dump());

        let kind = message["kind"].as_str().unwrap();

        match kind {
            "getTimeEntries" => self.send_time_entries(),
            "continueTimeEntry" => self.continue_time_entry(),
            "startTimeEntry" => self.start_time_entry(message),
            "stopTimeEntry" => self.stop_time_entry(),
            "adjustTime" => self.adjust_time(message),
            "sendWakeOnLan" => self.send_wake_on_lan(),
            "switchBoseDevices" => self.switch_bose_devices(message),
            _ => panic!("Unknown message kind: {}", kind),
        }
    }

    fn send_time_entries(&mut self) {
        let result = self
            .api_client
            .make_toggl_request("GET", "api/v9/me/time_entries", None);

        let response = match result {
            Err(_) => return self.send_error_message(),
            Ok(response) => response,
        };

        response.members().for_each(|member| {
            self.send_message(json::object! {
                kind: "timeEntry",
                timeEntry: {
                    description: member["description"].as_str().unwrap(),
                },
            });
        });

        self.send_success_message();
    }

    fn continue_time_entry(&mut self) {
        let result = self
            .api_client
            .make_toggl_request("GET", "api/v9/me/time_entries", None);

        let response = match result {
            Err(_) => return self.send_error_message(),
            Ok(response) => response,
        };

        let member = match response.members().next() {
            None => return self.send_error_message(),
            Some(member) => member,
        };

        let result = self.api_client.make_toggl_request(
            "POST",
            &format!("api/v9/workspaces/{}/time_entries", &env::var("TOGGL_WORKSPACE_ID").unwrap()),
            Some(&json::object! {
                created_with: "deskpi",
                project_id: i64::from_str_radix(&env::var("TOGGL_PROJECT_ID").unwrap(), 10).unwrap(),
                workspace_id: i64::from_str_radix(&env::var("TOGGL_WORKSPACE_ID").unwrap(), 10).unwrap(),
                start: chrono::Utc::now().to_rfc3339(),
                duration: -1,
                description: member["description"].as_str().unwrap(),
            }),
        );

        self.last_update = None;

        match result {
            Err(_) => self.send_error_message(),
            Ok(_) => self.send_success_message(),
        }
    }

    fn start_time_entry(&mut self, message: &json::JsonValue) {
        let result = self.api_client.make_toggl_request(
            "POST",
            &format!("api/v9/workspaces/{}/time_entries", &env::var("TOGGL_WORKSPACE_ID").unwrap()),
            Some(&json::object! {
                created_with: "deskpi",
                project_id: i64::from_str_radix(&env::var("TOGGL_PROJECT_ID").unwrap(), 10).unwrap(),
                workspace_id: i64::from_str_radix(&env::var("TOGGL_WORKSPACE_ID").unwrap(), 10).unwrap(),
                start: chrono::Utc::now().to_rfc3339(),
                duration: -1,
                description: message["timeEntry"]["description"].as_str().unwrap(),
            }),
        );

        self.last_update = None;

        match result {
            Err(_) => self.send_error_message(),
            Ok(_) => {
                self.thinkink.borrow_mut().send_message(json::object! {
                    kind: "startAnimation",
                });

                self.send_success_message()
            }
        }
    }

    fn stop_time_entry(&mut self) {
        let current_time_entry = match self.get_current_time_entry() {
            Err(_) => return self.send_error_message(),
            Ok(current_time_entry) => current_time_entry,
        };

        if current_time_entry.is_null() {
            return self.send_success_message();
        }

        let result = self.api_client.make_toggl_request(
            "PATCH",
            &format!(
                "api/v9/workspaces/{}/time_entries/{}/stop",
                &env::var("TOGGL_WORKSPACE_ID").unwrap(),
                current_time_entry["id"].as_i64().unwrap()
            ),
            Some(&json::object! {}),
        );

        self.last_update = None;

        match result {
            Err(_) => self.send_error_message(),
            Ok(_) => {
                self.thinkink.borrow_mut().send_message(json::object! {
                    kind: "stopAnimation",
                });

                self.send_success_message()
            }
        }
    }

    fn adjust_time(&mut self, message: &json::JsonValue) {
        let current_time_entry = match self.get_current_time_entry() {
            Err(_) => return self.send_error_message(),
            Ok(current_time_entry) => current_time_entry,
        };

        if current_time_entry.is_null() {
            return self.send_error_message();
        }

        let current_start =
            chrono::DateTime::parse_from_rfc3339(current_time_entry["start"].as_str().unwrap())
                .unwrap();
        let updated_start =
            current_start - chrono::Duration::minutes(message["minutes"].as_i64().unwrap());

        let result = self.api_client.make_toggl_request(
            "PUT",
            &format!(
                "api/v9/workspaces/{}/time_entries/{}",
                &env::var("TOGGL_WORKSPACE_ID").unwrap(),
                current_time_entry["id"].as_i64().unwrap()
            ),
            Some(&json::object! {
                start: updated_start.to_rfc3339()
            }),
        );

        self.last_update = None;

        match result {
            Err(_) => self.send_error_message(),
            Ok(_) => self.send_success_message(),
        }
    }

    fn send_wake_on_lan(&mut self) {
        self.api_client.send_wake_on_lan();
        self.send_success_message();
    }

    fn switch_bose_devices(&mut self, message: &json::JsonValue) {
        self.api_client.switch_bose_devices(
            message["devices"]
                .members()
                .map(|device| device.as_str().unwrap())
                .map(|device| macaddr::MacAddr6::from_str(device).unwrap())
                .collect::<Vec<macaddr::MacAddr6>>()
                .try_into()
                .unwrap(),
        );

        self.send_success_message();
    }

    fn get_current_time_entry(&self) -> Result<json::JsonValue, TogglError> {
        let response =
            self.api_client
                .make_toggl_request("GET", "api/v9/me/time_entries/current", None)?;

        Ok(response.to_owned())
    }

    fn send_success_message(&mut self) {
        self.send_message(json::object! { kind: "success" });
    }

    fn send_error_message(&mut self) {
        self.send_message(json::object! { kind: "error" });
    }

    fn send_message(&mut self, message: json::JsonValue) {
        log::debug!("-> {}", message.dump());
        self.output_buffer
            .extend_from_slice((message.dump() + "\n").as_bytes());
    }

    pub fn draw(&self, _context: &Context, _d: &mut RaylibDrawHandle) {}
}
