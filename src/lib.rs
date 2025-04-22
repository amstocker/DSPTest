pub mod input;
pub mod output;
pub mod analyze;

use std::sync::{Arc, Mutex};
use cpal::{traits::StreamTrait, Stream};
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoint};
use rtrb::{Consumer, Producer, RingBuffer};
use rustfft::num_complex::Complex32;
use rustfft::Fft;
use rustfft::{algorithm::Radix4, FftDirection};

use crate::input::{Event, Message, Widget};
use crate::output::{
    SAMPLE_RATE,
    OutputBuffer,
    build_output_stream
};
use crate::analyze::build_window_function;


const RINGBUFFER_CAPACITY: usize = 8;

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
    input_widgets: [Widget; IN],
    output_buffer: Arc<Mutex<OutputBuffer<SIZE, OUT>>>,
    output_buffer_time_series: [PlotPoint; SIZE],
    output_spectrum_complex: [Complex32; SIZE],
    output_spectrum_magnitude: [PlotPoint; SIZE],
    output_spectrum_filtered: [f64; SIZE],
    fft_window_func: [f32; SIZE],
    output_channel: usize,
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
            let widget = Widget::new(index);
            index += 1;
            widget
        });
        
        let mut output_buffer_plot = [PlotPoint::new(0.0, 0.0); SIZE];
        for i in 0..SIZE {
            output_buffer_plot[i].x = i as f64;
        }

        let mut output_spectrum_magnitude = [PlotPoint::new(0.0, 0.0); SIZE];
        for i in 0..SIZE {
            let f = ((i + 1) as f64 / SIZE as f64) * SAMPLE_RATE as f64;
            output_spectrum_magnitude[i].x = f.log2();
            // output_spectrum_magnitude[i].x = f;
        }

        Context {
            stream,
            sender: message_sender,
            receiver: event_receiver,
            input_widgets,
            output_buffer,
            output_buffer_time_series: output_buffer_plot,
            output_spectrum_complex: [Complex32::default(); SIZE],
            output_spectrum_magnitude,
            output_spectrum_filtered: [0.0; SIZE],
            fft_window_func: build_window_function(0.5),
            output_channel: 0,
            running: true
        }
    }

    fn process_output_buffer(&mut self) {
        let output_buffer = self.output_buffer.lock().unwrap();
        for i in 0..SIZE {
            self.output_buffer_time_series[i].y = output_buffer.buffer[self.output_channel][i] as f64;
            self.output_spectrum_complex[i] = Complex32 {
                re: self.fft_window_func[i] * output_buffer.buffer[self.output_channel][i],
                im: 0.0
            };
        }

        let fft  = Radix4::new(
            SIZE,
            FftDirection::Forward
        );

        fft.process(&mut self.output_spectrum_complex);

        let mut max_norm = 0.0;
        for i in 0..SIZE {
            let norm = self.output_spectrum_complex[i].norm() as f64;
            self.output_spectrum_filtered[i] += 0.1 * (norm - self.output_spectrum_filtered[i]);

            let norm_filtered = self.output_spectrum_filtered[i];
            self.output_spectrum_magnitude[i].y = norm_filtered;

            if norm_filtered > max_norm {
                max_norm = norm_filtered;
            }
        }

        for i in 0..SIZE {
            self.output_spectrum_magnitude[i].y /= max_norm;
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

        //egui::TopBottomPanel::top("Menu").show(ctx, |ui| {
        //    ui.label("[Audio Out options here]");
        //});

        egui::SidePanel::left("InputControls")
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Inputs");
                ui.separator();

                for widget in &mut self.input_widgets {
                    egui::Grid::new(widget.index)
                        .striped(true)
                        .show(ui, |ui| {
                            widget.render(ui, &mut self.sender);
                        });
                    ui.separator();
                }

                ui.heading("Options");
                ui.separator();
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
            });
            
            ui.separator();
            
            //Plot::new("Time")
            //    .show(ui, |plot_ui| {
            //        plot_ui.line(Line::new("Output", self.output_buffer_time_series.as_slice()));
            //    });

            Plot::new("Spectrum")
                .show(ui, |plot_ui| {
                    plot_ui.line(
                        Line::new("Output", &self.output_spectrum_magnitude[0..(SIZE / 2)])
                    );
                });
        });
        
        if self.running {
            ctx.request_repaint();
        }
    }
}
