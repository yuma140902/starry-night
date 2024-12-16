#![deny(rust_2018_idioms)]
#![deny(clippy::all)]
#![deny(clippy::nursery)]

pub mod camera;
mod game;
pub mod scene;
pub mod texture;
pub mod wgpu_wrapper;
mod winit_app;

pub use game::start_engine;
pub use game::Game;
