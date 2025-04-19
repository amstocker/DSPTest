use std::f32::consts::PI;


pub enum Wave {
    Sine,
    RampUp,
    RampDown,
    Square { pw: f32 }
}

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
            frequency: 0.1,
            scale: 1.0,
            offset: 0.0
        }
    }

    pub fn produce(&mut self) -> f32 {
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
                    0.0
                },
        };
        
        self.scale * sample + self.offset
    }
}