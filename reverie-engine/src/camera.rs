//! カメラに関するモジュール

use std::num::NonZero;

use nalgebra::{Matrix4, Point3, Scale3, Translation3, Vector3};

#[derive(Debug)]
pub struct Viewport {
    pub width: NonZero<u32>,
    pub height: NonZero<u32>,
}

#[derive(Debug)]
pub enum Camera {
    Orthographic(OrthographicCamera),
    Perspective(PerspectiveCamera),
}

impl Camera {
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        match self {
            Self::Orthographic(camera) => camera.get_matrix_world_to_render_coordinate(viewport),
            Self::Perspective(camera) => camera.get_matrix_world_to_render_coordinate(viewport),
        }
    }
}

impl From<OrthographicCamera> for Camera {
    fn from(value: OrthographicCamera) -> Self {
        Self::Orthographic(value)
    }
}

impl From<PerspectiveCamera> for Camera {
    fn from(value: PerspectiveCamera) -> Self {
        Self::Perspective(value)
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

#[derive(Debug)]
pub struct PerspectiveCamera {
    pub eye: Point3<f32>,
    pub target: Point3<f32>,
    pub up: Vector3<f32>,
    pub fov_y_rad: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl PerspectiveCamera {
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        // OpenGL and GLM uses a different coordinate system than wgpu, so we need to convert the projection matrix
        const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 0.5, 0.5, //
            0.0, 0.0, 0.0, 1.0, //
        );

        let view = Matrix4::look_at_lh(&self.eye, &self.target, &self.up);
        let proj = nalgebra_glm::perspective_fov_lh(
            self.fov_y_rad,
            viewport.width.get() as f32,
            viewport.height.get() as f32,
            self.z_near,
            self.z_far,
        );
        OPENGL_TO_WGPU_MATRIX * proj * view
    }
}
