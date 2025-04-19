use assert_no_alloc::*;


#[cfg(debug_assertions)]
#[global_allocator]
static A: AllocDisabler = AllocDisabler;


pub struct Channel {
    sample: f32
}

impl Channel {
    pub fn new() -> Self {
        Channel { sample: 0.0 }
    }

    pub fn consume(&mut self, sample: f32) {
        self.sample = sample;
    }
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