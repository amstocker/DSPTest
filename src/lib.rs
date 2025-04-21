pub mod input;
pub mod output;
pub mod analyze;

use std::sync::{Arc, Mutex};
use cpal::{traits::StreamTrait, Stream};
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoint, PlotPoints};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::input::{Event, Message, Widget};
use crate::output::{OUTPUT_BUFFER_SIZE, build_output_stream};


const RINGBUFFER_CAPACITY: usize = 8;

pub trait Module<const IN: usize, const OUT: usize>: 'static + Sized + Send {
    fn map_inputs(&mut self, input_buffer: &[f32; IN]);
    fn map_outputs(&mut self, output_buffer: &mut [f32; OUT]);
    
    fn run(self) -> eframe::Result {
        let context: Context<IN, OUT> = Context::new(self);

        context.run()
    }
}


pub struct Context<const IN: usize, const OUT: usize> {
    stream: Stream,
    sender: Producer<Message>,
    receiver: Consumer<Event<IN>>,
    input_widgets: [Widget; IN],
    output_buffer: Arc<Mutex<[[f32; OUTPUT_BUFFER_SIZE]; OUT]>>,
    output_buffer_plot: [[PlotPoint; OUTPUT_BUFFER_SIZE]; OUT],
    output_channel: usize
}

impl<const IN: usize, const OUT: usize> Context<IN, OUT> {
    pub fn new<M>(module: M) -> Self
    where
        M: 'static + Module<IN, OUT> + Send
    {
        let (
            message_sender,
            message_receiver
        ) = RingBuffer::new(RINGBUFFER_CAPACITY);

        let (
            event_sender,
            event_receiver
        ) = RingBuffer::new(RINGBUFFER_CAPACITY);

        let output_buffer = Arc::new(
            Mutex::new(
                [[0.0; OUTPUT_BUFFER_SIZE]; OUT]
            )
        );

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
        
        let mut output_buffer_plot = [[PlotPoint::new(0.0, 0.0); OUTPUT_BUFFER_SIZE]; OUT];
        for i in 0..OUTPUT_BUFFER_SIZE {
            for j in 0..OUT {
                output_buffer_plot[j][i] = PlotPoint::new(i as f64, 0.0);
            }
        }

        Context {
            stream,
            sender: message_sender,
            receiver: event_receiver,
            input_widgets,
            output_buffer,
            output_buffer_plot,
            output_channel: 0
        }
    }

    fn copy_output_buffer(&mut self) {
        let output_buffer = self.output_buffer.lock().unwrap();
        for j in 0..OUT {
            for i in 0..OUTPUT_BUFFER_SIZE {
                self.output_buffer_plot[j][i].y = output_buffer[j][i] as f64;
            }
        }
    }

    fn run(self) -> eframe::Result {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1024.0, 768.0]),
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

impl<const IN: usize, const OUT: usize> eframe::App for Context<IN, OUT> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(Event::State(input_channels)) = self.receiver.pop() {
            for i in 0..IN {
                self.input_widgets[i].set_model(input_channels[i]);
            }
        }

        egui::SidePanel::left("InputControls").show(ctx, |ui| {
            for widget in &mut self.input_widgets {
                widget.render(ui, &mut self.sender);
            }
        });
        
        self.copy_output_buffer();

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
            
            
            Plot::new("Time").show(ui, |plot_ui| {
                plot_ui.line(Line::new("Output", self.output_buffer_plot[self.output_channel].as_slice()));
            });
        });
        
        ctx.request_repaint();
    }
}
