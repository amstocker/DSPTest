use std::f32::consts::PI;


#[derive(PartialEq)]
pub enum PlotView {
    TimeSeries,
    Spectrum,
    Window
}

#[derive(PartialEq)]
pub enum TimeSeriesTracking {
    Static,
    Following
}

pub fn build_window_function<const N: usize>() -> [f32; N] {
    let mut window = [0.0; N];
    for i in 0..N {
        window[i] = 0.5 - 0.5 * ( (2.0 * PI * i as f32) / N as f32 ).cos();
    }
    window
}

