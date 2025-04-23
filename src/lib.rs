pub mod input;
pub mod output;
pub mod analyze;

use std::f32::consts::PI;
use std::sync::{Arc, Mutex};
use cpal::{traits::StreamTrait, Stream};
use eframe::egui;
use egui::Vec2b;
use egui_plot::{Line, Plot, PlotBounds, PlotPoint};
use rtrb::{Consumer, Producer, RingBuffer};
use rustfft::num_complex::Complex32;
use rustfft::Fft;
use rustfft::{algorithm::Radix4, FftDirection};

use crate::input::{Event, Message, Widget as InputWidget};
use crate::output::{
    SAMPLE_RATE,
    OutputBuffer,
    build_output_stream,
    Widget as OutputWidget
};
use crate::analyze::{build_window_function, PlotView};


const RINGBUFFER_CAPACITY: usize = 64;

pub trait Module<const IN: usize, const OUT: usize, const SIZE: usize>: 'static + Sized + Send {
    fn map_inputs(&mut self, input_buffer: &[f32; IN]);
    fn map_outputs(&mut self, output_buffer: &mut [f32; OUT]);
    
    fn run(self) -> eframe::Result {
        let context: Context<IN, OUT, SIZE> = Context::new(self);

        context.run()
    }
}


pub struct Context<const IN: usize, const OUT: usize, const SIZE: usize> {
    stream: Stream,
    sender: Producer<Message>,
    receiver: Consumer<Event<IN>>,
    input_widgets: [InputWidget; IN],
    output_widget: OutputWidget,
    output_buffer: Arc<Mutex<OutputBuffer<OUT, SIZE>>>,
    output_buffer_time_series: [PlotPoint; SIZE],
    output_buffer_freq_est: f32,
    output_buffer_phase: f32,
    output_spectrum_complex: [Complex32; SIZE],
    output_spectrum_magnitude: [PlotPoint; SIZE],
    output_spectrum_phase: [f32; SIZE],
    output_spectrum_filtered: [f64; SIZE],
    fft_window_func: [f32; SIZE],
    output_channel: usize,
    plot_view: PlotView,
    running: bool
}

impl<const IN: usize, const OUT: usize, const SIZE: usize> Context<IN, OUT, SIZE> {
    pub fn new<M>(module: M) -> Self
    where
        M: 'static + Module<IN, OUT, SIZE> + Send
    {
        let (
            message_sender,
            message_receiver
        ) = RingBuffer::new(RINGBUFFER_CAPACITY);

        let (
            event_sender,
            event_receiver
        ) = RingBuffer::new(RINGBUFFER_CAPACITY);

        let output_buffer = Arc::new(Mutex::new(
            OutputBuffer::new()
        ));

        let stream = build_output_stream(
            module,
            message_receiver,
            event_sender,
            output_buffer.clone()
        );

        let mut index = 0;
        let input_widgets = [(); IN].map(|_| {
            let widget = InputWidget::new(index);
            index += 1;
            widget
        });
        
        let mut output_buffer_plot = [PlotPoint::new(0.0, 0.0); SIZE];
        for i in 0..SIZE {
            output_buffer_plot[i].x = i as f64;
        }

        let mut output_spectrum_magnitude = [PlotPoint::new(0.0, 0.0); SIZE];
        for i in 0..SIZE {
            let f = (i + 1) as f64 / SIZE as f64;
            output_spectrum_magnitude[i].x = f.log2();
            // output_spectrum_magnitude[i].x = f;
        }

        Context {
            stream,
            sender: message_sender,
            receiver: event_receiver,
            input_widgets,
            output_widget: OutputWidget::new(),
            output_buffer,
            output_buffer_time_series: output_buffer_plot,
            output_buffer_freq_est: 0.0,
            output_buffer_phase: 0.0,
            output_spectrum_complex: [Complex32::default(); SIZE],
            output_spectrum_magnitude,
            output_spectrum_phase: [0.0; SIZE],
            output_spectrum_filtered: [0.0; SIZE],
            fft_window_func: build_window_function(),
            output_channel: 0,
            plot_view: PlotView::TimeSeries,
            running: true
        }
    }

    fn process_output_buffer(&mut self) {
        let mut output_buffer = self.output_buffer.lock().unwrap();
        
        // Process Spectrum
        let start = output_buffer.index;
        for i in 0..SIZE {
            self.output_spectrum_complex[i] = Complex32 {
                re: self.fft_window_func[i] * output_buffer.buffer[self.output_channel][(start + i) % SIZE],
                im: 0.0
            };
        }

        let fft  = Radix4::new(
            SIZE,
            FftDirection::Forward
        );

        fft.process(&mut self.output_spectrum_complex);

        let mut max_norm = 0.0;
        let mut max_norm_index = 0;
        let mut max_norm_phase_diff = 0.0;
        for i in 0..SIZE {
            let (norm, phase) = self.output_spectrum_complex[i].to_polar();
            //let norm_unfiltered = 20.0 * (r as f64).log10();
            let norm_unfiltered = norm as f64;
            
            self.output_spectrum_filtered[i] += 0.5 * (norm_unfiltered - self.output_spectrum_filtered[i]);
            let norm_filtered = self.output_spectrum_filtered[i];
            
            self.output_spectrum_magnitude[i].y = norm_filtered;

            let prev_phase = self.output_spectrum_phase[i];
            let phase_diff = phase - prev_phase;
            self.output_spectrum_phase[i] = phase;

            if norm_filtered > max_norm {
                max_norm = norm_filtered;
                max_norm_index = i;
                max_norm_phase_diff = phase_diff;
            }
        }

        for i in 0..SIZE {
            self.output_spectrum_magnitude[i].y /= max_norm;
        }


        // Process Time Series
        let dt = output_buffer.counter as f32;
        output_buffer.counter = 0;
        
        if dt != 0.0 {
            let freq_est = max_norm_index as f32 / SIZE as f32;
            let dp = max_norm_phase_diff;
            let mut phase = 0.0;
            let mut freq_prev = 0.0;
            self.output_buffer_freq_est = loop {
                let freq = (dp + phase) / (2.0 * PI * dt);
                if freq > freq_est {
                    if freq - freq_est < freq_est - freq_prev {
                        break freq;
                    } else {
                        break freq_prev;
                    };
                }
                freq_prev = freq;
                phase += 2.0 * PI;
            };
        }

        self.output_buffer_phase += self.output_buffer_freq_est * dt;
        while !(self.output_buffer_phase < 1.0) {
            self.output_buffer_phase -= 1.0;
        }

        let shift = (self.output_buffer_phase * SIZE as f32).round() as usize;
        println!("freq_est: {}, cycles: {}, dt: {}, shift: {}", self.output_buffer_freq_est, self.output_buffer_freq_est * dt, dt, shift);
        for i in 0..SIZE {
            self.output_buffer_time_series[i].y = 
                output_buffer.buffer[self.output_channel][(start + i).wrapping_sub(shift) % SIZE] as f64;
        }
    }

    fn run(self) -> eframe::Result {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 500.0]),
            ..Default::default()
        };

        self.stream.play().ok();

        eframe::run_native(
            "DSP Test",
            options,
            Box::new(|_cc| {
                Ok(Box::new(self))
            }),
        )
    }
}

impl<const IN: usize, const OUT: usize, const SIZE: usize> eframe::App for Context<IN, OUT, SIZE> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(Event::State(input_channels)) = self.receiver.pop() {
            for i in 0..IN {
                self.input_widgets[i].set_model(input_channels[i]);
            }
        }

        self.process_output_buffer();

        egui::SidePanel::left("InputControls")
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Inputs");
                ui.separator();
                for widget in &mut self.input_widgets {
                    widget.render(ui, &mut self.sender);
                    ui.separator();
                }

                ui.heading("Options");
                ui.separator();
                self.output_widget.render(ui);
            });
        

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Output Channel:");
                egui::ComboBox::from_id_salt("OutputSelect")
                    .selected_text(format!("{:?}", self.output_channel))
                    .show_ui(ui, |ui| {
                        for i in 0..OUT {
                            ui.selectable_value(&mut self.output_channel, i, i.to_string());
                        }
                    });

                ui.separator();

                ui.label("Plot View:");

                if ui.add(
                    egui::SelectableLabel::new(
                        self.plot_view == PlotView::TimeSeries,
                        "Time Series"
                    )
                ).clicked() {
                    self.plot_view = PlotView::TimeSeries;
                }

                if ui.add(
                    egui::SelectableLabel::new(
                        self.plot_view == PlotView::Spectrum,
                        "Spectrum"
                    )
                ).clicked() {
                    self.plot_view = PlotView::Spectrum;
                }

                if ui.add(
                    egui::SelectableLabel::new(
                        self.plot_view == PlotView::Window,
                        "Window"
                    )
                ).clicked() {
                    self.plot_view = PlotView::Window;
                }
            });
            
            ui.separator();
            
            match self.plot_view {
                PlotView::TimeSeries => Plot::new("Time Series")
                    .show(ui, |plot_ui| {
                        plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                            [0.0, -1.0],
                            [SIZE as f64, 1.0]
                        ));
                        plot_ui.set_auto_bounds(Vec2b::new(false, false));
                        plot_ui.line(
                            Line::new("Output", self.output_buffer_time_series.as_slice())
                        );
                    }),
                PlotView::Spectrum => Plot::new("Spectrum")
                    .show(ui, |plot_ui| {
                        plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                            [(1.0 / SIZE as f64).log2(), 0.0],
                            [(0.5_f64).log2(), 1.0]
                        ));
                        plot_ui.set_auto_bounds(Vec2b::new(false, false));
                        plot_ui.line(
                            Line::new("Output", &self.output_spectrum_magnitude[0..(SIZE / 2)])
                        );
                    }),
                PlotView::Window => Plot::new("Window")
                    .show(ui, |plot_ui| {
                        plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                            [0.0, 0.0],
                            [SIZE as f64, 1.0]
                        ));
                        plot_ui.set_auto_bounds(Vec2b::new(false, false));

                        let points = self.fft_window_func.iter().enumerate().map(|(x, &y)| {
                            [x as f64, y as f64]
                        }).collect::<Vec<_>>();
                        plot_ui.line(
                            Line::new("Output", points)
                        );
                    })
            }
            
        });
        
        if self.running {
            ctx.request_repaint();
        }
    }
}
