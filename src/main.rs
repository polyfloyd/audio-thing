extern crate sample;

mod pulse;

fn main() {
    let signal = pulse::source("blarp", 48000).unwrap();
    for frame in signal {
        print_sample(frame);
    }
}

fn print_sample(sm: [f32; 2]) {
    println!("{:?}, {:?}", sm[0], sm[1]);
}
