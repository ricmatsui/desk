use super::Context;
use raylib::prelude::*;
use serialport::{available_ports, SerialPort, SerialPortInfo, SerialPortType};
use std::{env, io, str};

pub struct MacroPad {
    serial_port: Option<Box<dyn SerialPort>>,
    input_buffer: Vec<u8>,
    output_buffer: Vec<u8>,
    api_client: Box<dyn super::ApiClient>,
}

pub fn init(api_client: Box<dyn super::ApiClient>) -> MacroPad {
    MacroPad {
        serial_port: None,
        input_buffer: Vec::new(),
        output_buffer: Vec::new(),
        api_client,
    }
}

pub fn open_macropad(macropad: &mut MacroPad) {
    if macropad.serial_port.is_some() {
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
        macropad.serial_port = Some(
            serialport::new(last.port_name.to_string(), 19200)
                .open()
                .unwrap(),
        );
        macropad.input_buffer.clear();
        macropad.output_buffer.clear();
    }
}

pub fn update(macropad: &mut MacroPad, context: &Context, _rl: &RaylibHandle) {
    update_buffers(macropad);
    process_input(macropad);
}

fn update_buffers(macropad: &mut MacroPad) {
    let port = match macropad.serial_port.as_mut() {
        Some(port) => port,
        None => return,
    };

    let mut disconnected = false;

    let mut input = [0; 256];
    match port.read(&mut input) {
        Ok(read_count) => {
            macropad
                .input_buffer
                .extend_from_slice(&input[0..read_count]);
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

    while macropad.output_buffer.len() > 0 && bytes_to_write < 256 && !disconnected {
        let output: Vec<u8> = macropad.output_buffer.iter().take(256).copied().collect();

        match port.write(&output) {
            Ok(write_count) => {
                macropad.output_buffer.drain(0..write_count);
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
        macropad.serial_port = None;
    }
}

fn process_input(macropad: &mut MacroPad) {
    while let Some(new_line_index) = macropad.input_buffer.iter().position(|&c| c == '\n' as u8) {
        let message_buffer: Vec<u8> = macropad.input_buffer.drain(0..=new_line_index).collect();
        let message = json::parse(str::from_utf8(&message_buffer).unwrap()).unwrap();

        process_message(macropad, &message);
    }
}

fn process_message(macropad: &mut MacroPad, message: &json::JsonValue) {
    log::debug!("<- {}", message.dump());

    let kind = message["kind"].as_str().unwrap();

    match kind {
        "getTimeEntries" => send_time_entries(macropad),
        "startTimeEntry" => start_time_entry(macropad, message),
        "stopTimeEntry" => stop_time_entry(macropad),
        "adjustTime" => adjust_time(macropad, message),
        _ => panic!("Unknown message kind: {}", kind),
    }
}

fn send_time_entries(macropad: &mut MacroPad) {
    let response = macropad
        .api_client
        .make_toggl_request("GET", "api/v8/time_entries", None);

    response.members().for_each(|member| {
        send_message(
            macropad,
            json::object! {
                kind: "timeEntry",
                timeEntry: {
                    description: member["description"].as_str().unwrap(),
                },
            },
        );
    });

    send_success_message(macropad);
}

fn start_time_entry(macropad: &mut MacroPad, message: &json::JsonValue) {
    macropad.api_client.make_toggl_request(
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

    send_success_message(macropad);
}

fn stop_time_entry(macropad: &mut MacroPad) {
    let current_time_entry = &get_current_time_entry(macropad)["data"];

    if !current_time_entry.is_null() {
        macropad.api_client.make_toggl_request(
            "PUT",
            &format!(
                "api/v8/time_entries/{}/stop",
                current_time_entry["id"].as_i64().unwrap()
            ),
            Some(&json::object! {}),
        );
    }

    send_success_message(macropad);
}

fn adjust_time(macropad: &mut MacroPad, message: &json::JsonValue) {
    let current_time_entry = &get_current_time_entry(macropad)["data"];

    if !current_time_entry.is_null() {
        let current_start =
            chrono::DateTime::parse_from_rfc3339(current_time_entry["start"].as_str().unwrap())
                .unwrap();
        let updated_start =
            current_start - chrono::Duration::minutes(message["minutes"].as_i64().unwrap());

        macropad.api_client.make_toggl_request(
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

    send_success_message(macropad);
}

fn get_current_time_entry(macropad: &MacroPad) -> json::JsonValue {
    macropad
        .api_client
        .make_toggl_request("GET", "api/v8/time_entries/current", None)
}

fn send_success_message(macropad: &mut MacroPad) {
    send_message(macropad, json::object! { kind: "success" });
}

fn send_message(macropad: &mut MacroPad, message: json::JsonValue) {
    log::debug!("-> {}", message.dump());
    macropad
        .output_buffer
        .extend_from_slice((message.dump() + "\n").as_bytes());
}

pub fn draw(macropad: &MacroPad, _context: &Context, d: &mut RaylibDrawHandle) {}
