use std::f32::consts::PI;

use egui::Ui;
use rtrb::Producer;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::output::ControlMessage;


#[derive(Clone, Copy, EnumIter)]
pub enum Wave {
    Sine,
    RampUp,
    RampDown,
    Square { pw: f32 },
    Const
}

impl PartialEq for Wave {
    fn eq(&self, other: &Self) -> bool {
        use core::mem::discriminant;

        discriminant(self) == discriminant(other)
    }
}

impl std::fmt::Display for Wave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Wave::Sine => write!(f, "Sine"),
            Wave::RampUp => write!(f, "Ramp Up"),
            Wave::RampDown => write!(f, "Ramp Down"),
            Wave::Square { .. } => write!(f, "Square"),
            Wave::Const => write!(f, "Const")
        }
    }
}

#[derive(Clone, Copy)]
pub struct Channel {
    wave: Wave,
    phase: f32,
    frequency: f32,
    scale: f32,
    offset: f32,
    enabled: bool
}

impl Channel {
    pub fn new() -> Self {
        Channel {
            wave: Wave::Sine,
            phase: 0.0,
            frequency: 0.0022,
            scale: 1.0,
            offset: 0.0,
            enabled: true
        }
    }

    pub fn handle_command(&mut self, command: Command) {
        match command {
            Command::SetWave(wave) =>
                self.wave = wave,
            Command::SetFrequency(frequency) =>
                self.frequency = frequency,
            Command::SetScale(scale) =>
                self.scale = scale,
            Command::SetOffset(offset) =>
                self.offset = offset,
            Command::SetEnabled =>
                self.enabled = true,
            Command::SetDisabled =>
                self.enabled = false
        }
    }

    pub fn process(&mut self) -> f32 {
        self.phase += self.frequency;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        if !self.enabled {
            return 0.0;
        }

        let sample = match self.wave {
            Wave::Sine =>
                (2.0 * PI * self.phase).sin(),
            Wave::RampUp =>
                2.0 * self.phase - 1.0,
            Wave::RampDown =>
                1.0 - 2.0 * self.phase,
            Wave::Square { pw } =>
                if self.phase < pw {
                    1.0
                } else {
                    -1.0
                },
            Wave::Const =>
                0.0
        };
        
        self.scale * sample + self.offset
    }
}

pub enum Event<const N: usize> {
    State([Channel; N])
}

pub enum Command {
    SetWave(Wave),
    SetFrequency(f32),
    SetScale(f32),
    SetOffset(f32),
    SetEnabled,
    SetDisabled
}


pub struct Widget<const N: usize> {
    models: [Channel; N]
}

impl<const N: usize> Widget<N> {
    pub fn new() -> Self {
        Widget {
            models: [Channel::new(); N]
        }
    }

    pub fn set_models(&mut self, models: [Channel; N]) {
        self.models = models;
    }

    fn render_channel(&mut self, index: usize, ui: &mut Ui, sender: &mut Producer<ControlMessage>) {
        egui::Grid::new(index)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Enabled:");
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Checkbox::new(&mut self.models[index].enabled, "")
                    ).changed() {
                        sender.push(ControlMessage::InputControl {
                            channel: index,
                            command: match self.models[index].enabled {
                                true => Command::SetEnabled,
                                false => Command::SetDisabled,
                            }
                        }).unwrap();
                    };
                });

                ui.end_row();

                ui.label("Frequency:");
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Slider::new(&mut self.models[index].frequency, 0.0..=5e-1)
                            .logarithmic(true)
                            .custom_formatter(|f, _| format!("{:.4}", f))
                    ).changed() {
                        sender.push(ControlMessage::InputControl {
                            channel: index,
                            command: Command::SetFrequency(self.models[index].frequency)
                        }).unwrap();
                    };
                });

                ui.end_row();

                ui.label("Scale:");
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Slider::new(&mut self.models[index].scale, 0.0..=1.0)
                            .custom_formatter(|f, _| format!("{:.2}", f))
                    ).changed() {
                        sender.push(ControlMessage::InputControl {
                            channel: index,
                            command: Command::SetScale(self.models[index].scale)
                        }).unwrap();
                    };
                });

                ui.end_row();

                ui.label("Offset:");
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Slider::new(&mut self.models[index].offset, -1.0..=1.0)
                            .custom_formatter(|f, _| format!("{:.2}", f))
                    ).changed() {
                        sender.push(ControlMessage::InputControl {
                            channel: index,
                            command: Command::SetOffset(self.models[index].offset)
                        }).unwrap();
                    };
                });

                ui.end_row();

                ui.label("Wave:");
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt(index)
                        .selected_text(format!("{}", self.models[index].wave.to_string()))
                        .show_ui(ui, |ui| {
                            for wave in Wave::iter() {
                                if ui.add(
                                    egui::SelectableLabel::new(
                                        self.models[index].wave == wave,
                                        wave.to_string()
                                    )
                                ).clicked() {
                                    self.models[index].wave = match wave {
                                        Wave::Square { .. } => Wave::Square { pw: 0.5 },
                                        other => other
                                    };
                                    sender.push(ControlMessage::InputControl {
                                        channel: index,
                                        command: Command::SetWave(self.models[index].wave)
                                    }).unwrap();
                                };
                            }
                        });
                    
                });

                ui.end_row();

                ui.label("Width:");
                ui.horizontal(|ui| {
                    if let Wave::Square { pw } = &mut self.models[index].wave {
                        if ui.add(
                            egui::Slider::new(pw, 0.0..=1.0)
                                .custom_formatter(|pw, _| format!("{:.0}%", 100.0 * pw))
                        ).changed() {
                            sender.push(ControlMessage::InputControl {
                                channel: index,
                                command: Command::SetWave(self.models[index].wave)
                            }).unwrap();
                        };
                    } else {
                        ui.label("—-");
                    }
                });

                ui.end_row();
            });
    }

    pub fn render(&mut self, ui: &mut Ui, sender: &mut Producer<ControlMessage>) {
        ui.heading("Inputs");
        ui.separator();
        for i in 0..N {
            self.render_channel(i, ui, sender);
            ui.separator();
        }
    }
}
