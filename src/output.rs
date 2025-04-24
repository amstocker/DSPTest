use std::sync::{Arc, Mutex};
use cpal::{Device, HostId, SampleFormat, Stream, StreamConfig};
use cpal::traits::{HostTrait, DeviceTrait};
use egui::Ui;
use rtrb::{Consumer, Producer};

use crate::Module;
use crate::input::{Channel, Event, Message};

pub const EVENT_UPDATE_INTERVAL: usize = 1024;
pub const SAMPLE_RATE: usize = 48_000;


pub struct OutputBuffer<const OUT: usize, const SIZE: usize> {
    pub buffer: [[f32; SIZE]; OUT],
    pub index: usize,
    pub counter: usize
}

impl<const OUT: usize, const SIZE: usize> OutputBuffer<OUT, SIZE> {
    pub fn new() -> Self {
        OutputBuffer {
            buffer: [[0.0; SIZE]; OUT],
            index: 0,
            counter: 0
        }
    }
}


pub fn build_output_stream<M, const IN: usize, const OUT: usize, const SIZE: usize>(
    mut module: M,
    mut receiver: Consumer<Message>,
    mut sender: Producer<Event<IN>>,
    output_buffer: Arc<Mutex<OutputBuffer<OUT, SIZE>>>
) -> Stream
where
    M: 'static + Module<IN, OUT, SIZE> + Send 
{
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    let config = device.default_output_config().unwrap();

    let channels = config.channels() as usize;
    assert!(OUT <= channels);
    assert!(config.sample_format() == SampleFormat::F32);

    let mut input_channels = [(); IN].map(|_| Channel::new());
    let mut input_buffer = [0.0; IN];

    device.build_output_stream(
        &config.config(),

        // Audio Callback
        move |data: &mut [f32], _| {

            // Handle incoming messages from UI Thread
            while let Ok(msg) = receiver.pop() {
                input_channels[msg.channel].handle_command(msg.command);
            }

            let mut output_buffer = output_buffer.lock().unwrap();
            for out_frame in data.chunks_mut(channels) {

                // Handle module inputs
                for i in 0..IN {
                    input_buffer[i] = input_channels[i].process();
                }
                module.map_inputs(&input_buffer);
                
                // Handle module outputs
                let mut outputs = [0.0; OUT];
                module.map_outputs(&mut outputs);

                // TODO: This doesn't actually make sense in general...
                for i in 0..OUT {
                    out_frame[i] = if input_channels[i].enabled() {
                        outputs[i]
                    } else {
                        0.0
                    };
                }

                // Copy to output buffer
                for i in 0..OUT {
                    let index = output_buffer.index;
                    output_buffer.buffer[i][index] = outputs[i];
                }
                output_buffer.index = (output_buffer.index + 1) % SIZE;
                output_buffer.counter += 1;
            }

            // Send state of inputs to main thread.  Ignore Errors.
            if output_buffer.index % EVENT_UPDATE_INTERVAL == 0 {
                sender.push(Event::State(input_channels)).ok();
            }
        },
        move |err| {
            panic!("{}", err);
        },
        None
    ).unwrap()
}


pub struct Widget {
    hosts: Vec<(HostId, String)>,
    selected_host_id: HostId,
    selected_host_name: String,
    devices: Vec<(Device, String)>,
    selected_device: Device,
    selected_device_index: usize,
    selected_device_name: String,
    config: StreamConfig
}

impl Widget {
    pub fn new() -> Self {
        let hosts = cpal::available_hosts().into_iter()
            .map(|host| (host, host.name().to_owned()))
            .collect();
        let selected_host = cpal::default_host();
        let selected_host_id = selected_host.id();
        let selected_host_name = selected_host_id.name().to_string();
        
        let devices: Vec<(Device, String)> = selected_host.devices().unwrap()
            .map(|dev| {
                let name = dev.name().unwrap();
                (dev, name)
            })
            .collect();
        let selected_device = selected_host.default_output_device().unwrap();
        let selected_device_name = selected_device.name().unwrap();
        let selected_device_index = devices.iter()
            .enumerate()
            .find(|(_, (_, dev_name))| *dev_name == selected_device_name)
            .unwrap()
            .0;
        
        let config = selected_device.default_output_config().unwrap().config();
        
        Widget {
            hosts,
            selected_host_id,
            selected_host_name,
            devices,
            selected_device,
            selected_device_index,
            selected_device_name,
            config
        }
    }

    pub fn render(&mut self, ui: &mut Ui) -> Option<Stream> {
        egui::Grid::new("OutputOptions")
            .striped(true)
            .show(ui, |ui| {
                ui.label("Host:");
                egui::ComboBox::from_id_salt("HostSelect")
                    .selected_text(format!("{}", self.selected_host_name))
                    .show_ui(ui, |ui| {
                        for (host, host_name) in &self.hosts {
                            if ui
                                .selectable_value(&mut self.selected_host_id, *host, host_name)
                                .clicked() {
                                    self.selected_host_id = host.clone();
                                    self.selected_host_name = self.selected_host_id.name().to_string();
                            }
                        }
                    });

                ui.end_row();

                ui.label("Device:");
                egui::ComboBox::from_id_salt("DeviceSelect")
                    .selected_text(format!("{}", self.selected_device_name))
                    .show_ui(ui, |ui| {
                        for (i, (_, device_name)) in self.devices.iter().enumerate() {
                            if ui
                                .selectable_value(&mut self.selected_device_index, i, device_name)
                                .clicked() {
                                    let device = &self.devices.get(self.selected_device_index).unwrap().0;
                                    self.selected_device = device.clone();
                                    self.selected_device_name = self.selected_device.name().unwrap();
                                    println!("device changed: {}", self.selected_device_name);
                            };
                        }
                    });

                ui.end_row();
            });

        None
    }
}

