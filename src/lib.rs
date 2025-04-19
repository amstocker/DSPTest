pub mod input;
pub mod output;


pub trait Module<const I: usize, const O: usize> {
    fn map_inputs(&mut self, input_buffer: &[f32; I]);
    fn map_outputs(&mut self, output_buffer: &mut [f32; O]);
    fn run (&mut self) {
        let mut context: Context<I, O> = Context::new();

        let mut input_buffer = [0.0; I];
        let mut output_buffer = [0.0; O];

        // TODO: Put EGUI loop here
        loop {
            for (
                input,
                sample
            ) in context.inputs.iter_mut().zip(input_buffer.iter_mut()) {
                *sample = input.produce();
            }

            self.map_inputs(&input_buffer);
            self.map_outputs(&mut output_buffer);

            for (
                output,
                &sample
            ) in context.outputs.iter_mut().zip(output_buffer.iter()) {
                output.consume(sample);
            }
        }
    }
}


// TODO: Put EGUI context in this struct?
pub struct Context<const I: usize, const O: usize> {
    inputs: [input::Channel; I],
    outputs: [output::Channel; O]
}

impl<const I: usize, const O: usize> Context<I, O> {
    pub fn new() -> Self {
        Context {
            inputs: [(); I].map(|_| input::Channel::new()),
            outputs: [(); O].map(|_| output::Channel::new())
        }
    }
}
