//! カメラに関するモジュール

use std::num::NonZero;

use nalgebra::{Matrix4, Scale3, Translation3};

#[derive(Debug)]
pub struct Viewport {
    pub width: NonZero<u32>,
    pub height: NonZero<u32>,
}

#[derive(Debug)]
pub enum Camera {
    Orthographic(OrthographicCamera),
}

impl Camera {
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        match self {
            Self::Orthographic(camera) => camera.get_matrix_world_to_render_coordinate(viewport),
        }
    }
}

impl From<OrthographicCamera> for Camera {
    fn from(value: OrthographicCamera) -> Self {
        Self::Orthographic(value)
    }
}

#[derive(Debug, Default)]
pub struct OrthographicCamera;

impl OrthographicCamera {
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        let width = viewport.width.get() as f32;
        let height = viewport.height.get() as f32;
        Translation3::from([-1.0, 1.0, 0.0]).to_homogeneous()
            * Scale3::new(2.0 / width, -2.0 / height, 1.0).to_homogeneous()
    }
}
