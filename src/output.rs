use std::sync::{Arc, Mutex};
use cpal::{Stream, SampleFormat};
use cpal::traits::{HostTrait, DeviceTrait};
use rtrb::{Consumer, Producer};

use crate::Module;
use crate::input::{Channel, Event, Message};

pub const OUTPUT_BUFFER_SIZE: usize = 2048;
pub const EVENT_UPDATE_INTERVAL: usize = 1024;


pub fn build_output_stream<M, const IN: usize, const OUT: usize>(
    mut module: M,
    mut receiver: Consumer<Message>,
    mut sender: Producer<Event<IN>>,
    output_buffer: Arc<Mutex<[[f32; OUTPUT_BUFFER_SIZE]; OUT]>>
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
    let mut input_channels = [(); IN].map(|_| Channel::new());
    let mut input_buffer = [0.0; IN];

    device.build_output_stream(
        &config.config(),

        // Audio Callback
        move |data: &mut [f32], _| {

            // Handle incoming messages from UI Thread
            while let Ok(msg) = receiver.pop() {
                input_channels[msg.channel].handle_command(msg.command);
            }

            let mut output_buffer = output_buffer.lock().unwrap();
            for out_frame in data.chunks_mut(channels) {
                
                // Handle module inputs
                for i in 0..IN {
                    input_buffer[i] = input_channels[i].process();
                }
                module.map_inputs(&input_buffer);
                
                // Handle module outputs
                let mut outputs = (&mut out_frame[0..OUT]).try_into().unwrap();
                module.map_outputs(&mut outputs);

                // Copy to output buffer
                for i in 0..OUT {
                    output_buffer[i][buffer_index] = outputs[i];
                }
                buffer_index = (buffer_index + 1) % OUTPUT_BUFFER_SIZE;
            }

            // Send state of inputs to main thread.  Ignore Errors.
            if buffer_index % EVENT_UPDATE_INTERVAL == 0 {
                sender.push(Event::State(input_channels)).ok();
            }
        },
        move |err| {
            panic!("{}", err);
        },
        None
    ).unwrap()
}


#[cfg(test)]
mod tests {

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