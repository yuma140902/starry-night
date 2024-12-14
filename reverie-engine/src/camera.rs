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
pub struct OrthographicCamera {
    pub eye: Point3<f32>,
    pub target: Point3<f32>,
    pub up: Vector3<f32>,
    // カメラの描画範囲の高さの半分の値
    pub size: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl OrthographicCamera {
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        let view = Matrix4::look_at_lh(&self.eye, &self.target, &self.up);

        let aspect_ratio = viewport.width.get() as f32 / viewport.height.get() as f32;
        let half_height = self.size;
        let half_width = half_height * aspect_ratio;
        let left = -half_width;
        let right = half_width;
        let bottom = -half_height;
        let top = half_height;

        let proj = nalgebra_glm::ortho_lh_zo(left, right, bottom, top, self.z_near, self.z_far);

        proj * view
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
