use super::Context;
use raylib::prelude::*;
use serialport::{available_ports, SerialPort, SerialPortInfo, SerialPortType};
use std::sync::mpsc;
use std::thread;
use std::{io, str};

pub struct CircuitPlayground {
    serial_port: Option<Box<dyn SerialPort>>,
    input_buffer: Vec<u8>,
    output_buffer: Vec<u8>,
    metrics_thread: std::thread::JoinHandle<()>,
    metrics_request_tx: Option<mpsc::SyncSender<json::JsonValue>>,
    last_metrics_submitted: Option<f64>,
}

impl CircuitPlayground {
    pub fn new(
        rl: &mut raylib::RaylibHandle,
        thread: &raylib::RaylibThread,
        api_client: std::sync::Arc<dyn super::ApiClient>,
    ) -> Self {
        let (metrics_thread, metrics_request_tx) = start_metrics_thread(api_client.clone());

        Self {
            serial_port: None,
            input_buffer: Vec::new(),
            output_buffer: Vec::new(),
            metrics_thread,
            metrics_request_tx: Some(metrics_request_tx),
            last_metrics_submitted: None,
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
                    return usb_info.vid == 0x239A && usb_info.pid == 0x8019
                }
                _ => false,
            })
            .collect();

        for port_info in matching_port_infos {
            let mut port = match serialport::new(port_info.port_name.to_string(), 115200).open() {
                Ok(port) => port,
                Err(_) => continue,
            };

            let mut buffer = [0; 2];

            let read_count = match port.read(&mut buffer) {
                Ok(count) => count,
                Err(_) => continue,
            };

            if &buffer[0..read_count] != b"c\n" {
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
        self.process_input(rl);
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

    fn process_input(&mut self, rl: &RaylibHandle) {
        while let Some(new_line_index) = self.input_buffer.iter().position(|&c| c == '\n' as u8) {
            let message_buffer: Vec<u8> = self.input_buffer.drain(0..=new_line_index).collect();

            let message_string = match str::from_utf8(&message_buffer) {
                Ok(message_string) => message_string,
                Err(_) => {
                    log::warn!("= invalid utf-8 message: {:?}", message_buffer);
                    continue;
                }
            };

            if message_string == "c\n" {
                continue;
            }

            if message_string.starts_with("ra1") {
                let value = message_string[3..message_string.len() - 1]
                    .parse::<u32>()
                    .unwrap();

                let timestamp = chrono::Utc::now().timestamp();

                if self.last_metrics_submitted.is_none()
                    || rl.get_time() - self.last_metrics_submitted.unwrap() > 60.0
                {
                    self.metrics_request_tx
                        .as_mut()
                        .unwrap()
                        .send(json::object! {
                            series: [
                                {
                                    metric: "soil.capacitance",
                                    type: 3,
                                    points: [{ timestamp: timestamp, value: value, }],
                                    resources: [{ name: "deskpi", type: "host" }],
                                    tags: ["input:a1"],
                                }
                            ]
                        })
                        .unwrap();

                    self.last_metrics_submitted = Some(rl.get_time());
                }
                continue;
            }
        }
    }

    pub fn draw(&self, _context: &Context, _d: &mut RaylibDrawHandle) {}

    pub fn shutdown(mut self) {
        self.metrics_request_tx = None;
        self.metrics_thread.join().unwrap();
    }
}

fn start_metrics_thread(
    api_client: std::sync::Arc<dyn super::ApiClient>,
) -> (
    std::thread::JoinHandle<()>,
    mpsc::SyncSender<json::JsonValue>,
) {
    let (metrics_request_tx, metrics_request_rx) = mpsc::sync_channel::<json::JsonValue>(100);

    let metrics_thread = thread::spawn(move || {
        while let Ok(metrics) = metrics_request_rx.recv() {
            api_client.submit_metrics(metrics);
        }
    });

    (metrics_thread, metrics_request_tx)
}
