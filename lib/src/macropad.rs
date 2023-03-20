use super::Context;
use raylib::prelude::*;
use serialport::{available_ports, SerialPort, SerialPortInfo, SerialPortType};
use std::rc::Rc;
use std::{env, io, str};

pub struct MacroPad {
    serial_port: Option<Box<dyn SerialPort>>,
    input_buffer: Vec<u8>,
    output_buffer: Vec<u8>,
    api_client: Rc<dyn super::ApiClient>,
}

impl MacroPad {
    pub fn new(api_client: Rc<dyn super::ApiClient>) -> Self {
        Self {
            serial_port: None,
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            api_client,
        }
    }

    pub fn open_serial(&mut self) {
        if self.serial_port.is_some() {
            return;
        }

        let mut matching_port_infos: Vec<SerialPortInfo> = available_ports()
            .unwrap()
            .into_iter()
            .filter(|port_info| match port_info.port_type {
                SerialPortType::UsbPort(ref usb_info) => {
                    return usb_info.vid == 0x239A && usb_info.pid == 0x8108
                }
                _ => false,
            })
            .collect();

        matching_port_infos.sort_by(|a, b| a.port_name.cmp(&b.port_name));

        if let Some(last) = matching_port_infos.last() {
            self.serial_port = Some(
                serialport::new(last.port_name.to_string(), 19200)
                    .open()
                    .unwrap(),
            );
            self.input_buffer.clear();
            self.output_buffer.clear();
        }
    }

    pub fn update(&mut self, _context: &Context, _rl: &RaylibHandle) {
        self.update_buffers();
        self.process_input();
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
                    disconnected = true;
                }
            }
        }

        let bytes_to_write = match port.bytes_to_write() {
            Ok(bytes) => bytes,
            Err(_) => {
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
                        disconnected = true;
                    }

                    break;
                }
            }
        }

        if disconnected {
            self.serial_port = None;
        }
    }

    fn process_input(&mut self) {
        while let Some(new_line_index) = self.input_buffer.iter().position(|&c| c == '\n' as u8) {
            let message_buffer: Vec<u8> = self.input_buffer.drain(0..=new_line_index).collect();
            let message = json::parse(str::from_utf8(&message_buffer).unwrap()).unwrap();

            self.process_message(&message);
        }
    }

    fn process_message(&mut self, message: &json::JsonValue) {
        log::debug!("<- {}", message.dump());

        let kind = message["kind"].as_str().unwrap();

        match kind {
            "getTimeEntries" => self.send_time_entries(),
            "startTimeEntry" => self.start_time_entry(message),
            "stopTimeEntry" => self.stop_time_entry(),
            "adjustTime" => self.adjust_time(message),
            _ => panic!("Unknown message kind: {}", kind),
        }
    }

    fn send_time_entries(&mut self) {
        let response = self
            .api_client
            .make_toggl_request("GET", "api/v8/time_entries", None);

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

    fn start_time_entry(&mut self, message: &json::JsonValue) {
        self.api_client.make_toggl_request(
            "POST",
            "api/v8/time_entries/start",
            Some(&json::object! {
                time_entry: {
                    created_with: "deskpi",
                    pid: i64::from_str_radix(&env::var("TOGGL_PROJECT_ID").unwrap(), 10).unwrap(),
                    wid: i64::from_str_radix(&env::var("TOGGL_WORKSPACE_ID").unwrap(), 10).unwrap(),
                    description: message["timeEntry"]["description"].as_str().unwrap(),
                }
            }),
        );

        self.send_success_message();
    }

    fn stop_time_entry(&mut self) {
        let current_time_entry = &self.get_current_time_entry()["data"];

        if !current_time_entry.is_null() {
            self.api_client.make_toggl_request(
                "PUT",
                &format!(
                    "api/v8/time_entries/{}/stop",
                    current_time_entry["id"].as_i64().unwrap()
                ),
                Some(&json::object! {}),
            );
        }

        self.send_success_message();
    }

    fn adjust_time(&mut self, message: &json::JsonValue) {
        let current_time_entry = &self.get_current_time_entry()["data"];

        if !current_time_entry.is_null() {
            let current_start =
                chrono::DateTime::parse_from_rfc3339(current_time_entry["start"].as_str().unwrap())
                    .unwrap();
            let updated_start =
                current_start - chrono::Duration::minutes(message["minutes"].as_i64().unwrap());

            self.api_client.make_toggl_request(
                "PUT",
                &format!(
                    "api/v8/time_entries/{}",
                    current_time_entry["id"].as_i64().unwrap()
                ),
                Some(&json::object! {
                    time_entry: {
                        start: updated_start.to_rfc3339()
                    }
                }),
            );
        }

        self.send_success_message();
    }

    fn get_current_time_entry(&self) -> json::JsonValue {
        self.api_client
            .make_toggl_request("GET", "api/v8/time_entries/current", None)
    }

    fn send_success_message(&mut self) {
        self.send_message(json::object! { kind: "success" });
    }

    fn send_message(&mut self, message: json::JsonValue) {
        log::debug!("-> {}", message.dump());
        self.output_buffer
            .extend_from_slice((message.dump() + "\n").as_bytes());
    }

    pub fn draw(&self, _context: &Context, _d: &mut RaylibDrawHandle) {}
}
