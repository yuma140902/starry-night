use std::time::{Duration, Instant};

use re::gl;
use re::ReverieEngine;
use reverie_engine as re;

pub fn main() {
    let engine = ReverieEngine::new();
    let mut window = engine.create_window();
    let context = window.create_context();
    context.make_current();
    let gl = context.gl();

    let mut start = Instant::now();

    while !window.process_event() {
        let elapsed = start.elapsed();
        if elapsed > Duration::from_millis(16) {
            println!(
                "{} FPS",
                Duration::from_secs(1).as_nanos() as f64 / elapsed.as_nanos() as f64
            );
            unsafe {
                gl.ClearColor(1.0, 0.0, 1.0, 1.0);
                gl.Clear(gl::COLOR_BUFFER_BIT);
            }
            context.swap_buffers();
            start = Instant::now();
        }
    }
}
