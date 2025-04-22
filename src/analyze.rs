use std::f32::consts::PI;


#[derive(PartialEq)]
pub enum PlotView {
    TimeSeries,
    Spectrum
}

pub fn build_window_function<const N: usize>(a: f32) -> [f32; N] {
    let mut window = [0.0; N];
    for i in 0..N {
        window[i] = a - (1.0 - a) * ( (2.0 * PI * i as f32) / N as f32 ).cos();
    }
    window
}

