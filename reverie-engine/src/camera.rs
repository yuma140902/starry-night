//! カメラに関するモジュール

use std::num::NonZero;

#[derive(Debug)]
pub struct Viewport {
    pub width: NonZero<u32>,
    pub height: NonZero<u32>,
}
