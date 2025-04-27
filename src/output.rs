use std::sync::{Arc, Mutex};
use cpal::{Device, HostId, SampleFormat, Stream, StreamConfig};
use cpal::traits::{HostTrait, DeviceTrait};
use egui::Ui;
use rtrb::{Consumer, Producer};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::Module;
use crate::input;

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


#[derive(Clone, Copy, Default, PartialEq, EnumIter)]
pub enum OutputMap {
    #[default]
    Both,
    Left,
    Right
}

impl std::fmt::Display for OutputMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputMap::Both => write!(f, "L+R"),
            OutputMap::Left => write!(f, "L"),
            OutputMap::Right => write!(f, "R")
        }
    }
}

pub enum Command {
    SetMap(OutputMap),
    SetVolume(f32),
    SetEnabled,
    SetDisabled
}


#[derive(Clone, Copy)]
pub struct Channel {
    output_map: OutputMap,
    volume: f32,
    enabled: bool
}

impl Channel {
    pub fn new() -> Self {
        Channel {
            output_map: OutputMap::default(),
            volume: 0.5,
            enabled: true,
        }
    }

    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::SetMap(output_map) =>
                self.output_map = output_map,
            Command::SetVolume(volume) =>
                self.volume = volume,
            Command::SetEnabled =>
                self.enabled = true,
            Command::SetDisabled =>
                self.enabled = false,
        }
    }
}


pub enum ControlMessage {
    OutputControl {
        channel: usize,
        command: Command
    },
    InputControl {
        channel: usize,
        command: input::Command
    }
}


pub fn build_output_stream<M, const IN: usize, const OUT: usize, const SIZE: usize>(
    mut module: M,
    mut receiver: Consumer<ControlMessage>,
    mut sender: Producer<input::Event<IN>>,
    output_buffer: Arc<Mutex<OutputBuffer<OUT, SIZE>>>
) -> Stream
where
    M: 'static + Module<IN, OUT> + Send 
{
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    let config = device.default_output_config().unwrap();

    let channels = config.channels() as usize;
    assert!(channels >= 2);
    assert!(config.sample_format() == SampleFormat::F32);

    let mut input_channels = [(); IN].map(|_| input::Channel::new());
    let mut output_channels = [(); OUT].map(|_| Channel::new());

    device.build_output_stream(
        &config.config(),

        // Audio Callback
        move |data: &mut [f32], _| {

            // Handle incoming messages from UI Thread
            while let Ok(message) = receiver.pop() {
                match message {
                    ControlMessage::InputControl { channel, command } => {
                        input_channels[channel].handle_command(command);
                    },
                    ControlMessage::OutputControl { channel, command  } => {
                        output_channels[channel].handle_command(command);
                    },
                }
            }

            let mut output_buffer = output_buffer.lock().unwrap();
            for out_frame in data.chunks_mut(channels) {

                // Handle module inputs
                let mut inputs = [0.0; IN];
                for i in 0..IN {
                    inputs[i] = input_channels[i].process();
                }
                module.map_inputs(&inputs);
                
                // Handle module outputs
                let mut outputs = [0.0; OUT];
                module.map_outputs(&mut outputs);

                out_frame[0] = 0.0;
                out_frame[1] = 0.0;
                for i in 0..OUT {
                    if !output_channels[i].enabled {
                        continue;
                    }

                    let scale = output_channels[i].volume;
                    match output_channels[i].output_map {
                        OutputMap::Both => {
                            out_frame[0] += scale * outputs[i];
                            out_frame[1] += scale * outputs[i];
                        },
                        OutputMap::Left => {
                            out_frame[0] += scale * outputs[i];
                        },
                        OutputMap::Right => {
                            out_frame[1] += scale * outputs[i];
                        }
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
                sender.push(input::Event::State(input_channels)).ok();
            }
        },
        move |err| {
            panic!("{}", err);
        },
        None
    ).unwrap()
}


pub struct Widget<const N: usize> {
    hosts: Vec<(HostId, String)>,
    selected_host_id: HostId,
    selected_host_name: String,
    devices: Vec<(Device, String)>,
    selected_device: Device,
    selected_device_index: usize,
    selected_device_name: String,
    config: StreamConfig,
    models: [Channel; N]
}

impl<const N: usize> Widget<N> {
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
            config,
            models: [Channel::new(); N]
        }
    }

    pub fn render(&mut self, ui: &mut Ui, sender: &mut Producer<ControlMessage>) -> Option<Stream> {
        ui.heading("Outputs");
        ui.separator();
        
        for index in 0..N {
            egui::Grid::new(index + 1000)
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Audio:");
                    ui.horizontal(|ui| {
                        if ui.add(
                            egui::Checkbox::new(&mut self.models[index].enabled, "")
                        ).changed() {
                            sender.push(ControlMessage::OutputControl {
                                channel: index,
                                command: match self.models[index].enabled {
                                    true => Command::SetEnabled,
                                    false => Command::SetDisabled,
                                }
                            }).unwrap();
                        };
                    });

                    ui.end_row();

                    ui.label("Map:");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(index + 2000)
                            .selected_text(self.models[index].output_map.to_string())
                            .show_ui(ui, |ui| {
                                for map in OutputMap::iter() {
                                    if ui.add(
                                        egui::SelectableLabel::new(
                                            self.models[index].output_map == map,
                                            map.to_string()
                                        )
                                    ).clicked() {
                                        self.models[index].output_map = map;
                                        sender.push(ControlMessage::OutputControl {
                                            channel: index,
                                            command: Command::SetMap(map)
                                        }).unwrap();
                                    };
                                }
                            });
                    });

                    ui.end_row();

                    ui.label("Volume:");
                    ui.horizontal(|ui| {
                    if ui.add(
                        egui::Slider::new(&mut self.models[index].volume, 0.0..=1.0)
                            .custom_formatter(|f, _| format!("{:.2}%", 100.0 * f))
                    ).changed() {
                        sender.push(ControlMessage::OutputControl {
                            channel: index,
                            command: Command::SetVolume(self.models[index].volume)
                        }).unwrap();
                    };
                });
                });
            
            ui.separator();
        }

        ui.heading("Options");
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

