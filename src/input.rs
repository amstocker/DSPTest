use std::f32::consts::PI;

use egui::Ui;
use rtrb::Producer;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;


#[derive(Clone, Copy, EnumIter)]
pub enum Wave {
    Sine,
    RampUp,
    RampDown,
    Square { pw: f32 }
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
        }
    }
}

#[derive(Clone, Copy)]
pub struct Channel {
    wave: Wave,
    phase: f32,
    frequency: f32,
    scale: f32,
    offset: f32
}

impl Channel {
    pub fn new() -> Self {
        Channel {
            wave: Wave::Sine,
            phase: 0.0,
            frequency: 0.001,
            scale: 1.0,
            offset: 0.0
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
                self.offset = offset
        }
    }

    pub fn process(&mut self) -> f32 {
        self.phase += self.frequency;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
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
    SetOffset(f32)
}

pub struct Message {
    pub channel: usize,
    pub command: Command
}


pub struct Widget {
    pub index: usize,
    model: Channel
}

impl Widget {
    pub fn new(index: usize) -> Self {
        Widget {
            index,
            model: Channel::new()
        }
    }

    pub fn set_model(&mut self, model: Channel) {
        self.model = model;
    }

    pub fn render(&mut self, ui: &mut Ui, sender: &mut Producer<Message>) {
        ui.label("Frequency:");
        ui.horizontal(|ui| {
            if ui.add(
                egui::Slider::new(&mut self.model.frequency, 1e-5..=5e-1)
                    .logarithmic(true)
                    .custom_formatter(|f, _| format!("{:.4}", f))
            ).changed() {
                sender.push(Message {
                    channel: self.index,
                    command: Command::SetFrequency(self.model.frequency)
                }).unwrap();
            };
        });

        ui.end_row();

        ui.label("Scale:");
        ui.horizontal(|ui| {
            if ui.add(
                egui::Slider::new(&mut self.model.scale, 0.0..=1.0)
                    .custom_formatter(|f, _| format!("{:.2}", f))
            ).changed() {
                sender.push(Message {
                    channel: self.index,
                    command: Command::SetScale(self.model.scale)
                }).unwrap();
            };
        });

        ui.end_row();

        ui.label("Offset:");
        ui.horizontal(|ui| {
            if ui.add(
                egui::Slider::new(&mut self.model.offset, -1.0..=1.0)
                    .custom_formatter(|f, _| format!("{:.2}", f))
            ).changed() {
                sender.push(Message {
                    channel: self.index,
                    command: Command::SetOffset(self.model.offset)
                }).unwrap();
            };
        });

        ui.end_row();

        ui.label("Wave:");
        ui.horizontal(|ui| {
            for wave in Wave::iter() {
                if ui.add(
                    egui::SelectableLabel::new(
                        self.model.wave == wave,
                        wave.to_string()
                    )
                ).clicked() {
                    self.model.wave = match wave {
                        Wave::Square { .. } => Wave::Square { pw: 0.5 },
                        other => other
                    };
                    sender.push(Message {
                        channel: self.index,
                        command: Command::SetWave(self.model.wave)
                    }).unwrap();
                };
            }
        });

        ui.end_row();

        ui.label("Width:");
        ui.horizontal(|ui| {
            if let Wave::Square { pw } = &mut self.model.wave {
                if ui.add(
                    egui::Slider::new(pw, 0.0..=1.0)
                        .custom_formatter(|pw, _| format!("{:.0}%", 100.0 * pw))
                ).changed() {
                    sender.push(Message {
                        channel: self.index,
                        command: Command::SetWave(self.model.wave)
                    }).unwrap();
                };
            }
        });

        ui.end_row();
    }
}
