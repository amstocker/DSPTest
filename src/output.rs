use assert_no_alloc::*;

use cpal::{Stream, SampleFormat};
use cpal::traits::{HostTrait, DeviceTrait};
use rtrb::Consumer;

use crate::{Module, OUTPUT_BUFFER_SIZE};

#[cfg(debug_assertions)]
#[global_allocator]
static A: AllocDisabler = AllocDisabler;


pub fn build_output_stream<M, const IN: usize, const OUT: usize>(
    mut module: M,
    mut receiver: Consumer<[f32; IN]>,
    output_buffer_unsafe_copy: &'static mut [[f32; OUTPUT_BUFFER_SIZE]; OUT]
) -> Stream
where
    M: 'static + Module<IN, OUT> + Send 
{
    let host = cpal::default_host();
    let device = host.default_output_device().unwrap();
    let config = device.default_output_config().unwrap();

    let channels = config.channels() as usize;
    assert!(OUT <= channels);
    assert!(config.sample_format() == SampleFormat::F32);

    let mut buffer_index = 0;
    device.build_output_stream(
        &config.config(),
        move |data: &mut [f32], _| {
            assert_no_alloc(|| {
                if let Ok(input_buffer) = receiver.pop() {
                    module.map_inputs(&input_buffer);
                }

                for out_frame in data.chunks_mut(channels) {
                    let mut output_buffer = (&mut out_frame[0..OUT]).try_into().unwrap();
                    module.map_outputs(&mut output_buffer);

                    for i in 0..OUT {
                        output_buffer_unsafe_copy[i][buffer_index] = output_buffer[i];
                    }
                    buffer_index = (buffer_index + 1) % OUTPUT_BUFFER_SIZE;
                }
            });
        },
        move |err| {
            panic!("{}", err);
        },
        None
    ).unwrap()
}


#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn list_hosts() {
        use cpal::traits::{HostTrait, DeviceTrait};

        let x = cpal::available_hosts();

        for host in x {
            println!("Host Name: {:?}", host.name());

            let y = cpal::host_from_id(host);
            let devices = y.unwrap().output_devices();
            for d in devices.unwrap() {
                println!("\tDevice: {:?}", d.name().unwrap());
                let ic = d.supported_input_configs().unwrap();
                for c in ic {
                    println!("\t\tInput Config: {:?}", c);
                }

                let oc = d.supported_output_configs().unwrap();
                for c in oc {
                    println!("\t\tOutput Config: {:?}", c);
                }

                let dd = d.default_output_config().unwrap();
                println!("Default Output Config: {:?}", dd.config());
            }
        }
    }
}