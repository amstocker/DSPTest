pub mod input;
pub mod output;

use std::sync::{Arc, Mutex};
use cpal::{traits::StreamTrait, Stream};
use eframe::egui;
use output::build_output_stream;
use rtrb::{Producer, RingBuffer};

use crate::input::Channel;


const OUTPUT_BUFFER_SIZE: usize = 2048;


pub trait Module<const IN: usize, const OUT: usize>: 'static + Sized + Send {
    fn map_inputs(&mut self, input_buffer: &[f32; IN]);
    fn map_outputs(&mut self, output_buffer: &mut [f32; OUT]);
    
    fn run(self) -> eframe::Result {
        let context: Context<IN, OUT> = Context::new(self);

        context.run()
    }
}


pub struct Context<const IN: usize, const OUT: usize> {
    inputs: [Channel; IN],
    sender: Producer<[f32; IN]>,
    stream: Stream,
    output_buffer: Arc<Mutex<[[f32; OUTPUT_BUFFER_SIZE]; OUT]>>
}

impl<const IN: usize, const OUT: usize> Context<IN, OUT> {
    pub fn new<M>(module: M) -> Self
    where
        M: 'static + Module<IN, OUT> + Send
    {
        let (
            sender,
            receiver
        ) = RingBuffer::new(8);

        let output_buffer = Arc::new(
            Mutex::new(
                [[0.0; OUTPUT_BUFFER_SIZE]; OUT]
            )
        );

        let stream = build_output_stream(
            module,
            receiver,
            output_buffer.clone()
        );

        Context {
            inputs: [(); IN].map(|_| Channel::new()),
            sender,
            stream,
            output_buffer
        }
    }

    fn run(self) -> eframe::Result {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
            ..Default::default()
        };

        println!("starting stream ...");
        self.stream.play().ok();

        println!("starting gui ...");
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
        println!("updating frame ...");
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.label("Hello");
        });

        let mut input_buffer = [0.0; IN];
        for i in 0..IN {
            input_buffer[i] = self.inputs[i].produce();
        }
        self.sender.push(input_buffer).ok();
    }
}
