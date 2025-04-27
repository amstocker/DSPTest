use dsp_test::Module;


struct Through<const N: usize> {
    values: [f32; N]
}

impl<const N: usize> Through<N> {
    pub fn new() -> Self {
        Through { values: [0.0; N] }
    }
}

impl<const N: usize> Module<N, N> for Through<N> {
    fn map_inputs(&mut self, input_buffer: &[f32; N]) {
        self.values.copy_from_slice(input_buffer);
    }

    fn map_outputs(&mut self, output_buffer: &mut [f32; N]) {
        output_buffer.copy_from_slice(&self.values);
    }
}

pub fn main() {
    let module = Through::<2>::new();
    module.run().unwrap();
}