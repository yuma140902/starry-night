//! カメラに関するモジュール

use std::num::NonZero;

use nalgebra::{Matrix4, Point3, Translation3, UnitQuaternion, Vector3};

use crate::scene::TransformComponent;

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
    pub transform: TransformComponent,
    pub fov_y_rad: f32,
    pub z_near: f32,
    pub z_far: f32,
}

impl PerspectiveCamera {
    pub fn new(
        eye: &Point3<f32>,
        target: &Point3<f32>,
        up: &Vector3<f32>,
        fov_y_rad: f32,
        z_near: f32,
        z_far: f32,
    ) -> Self {
        let dir = target - eye;
        let rotation = UnitQuaternion::look_at_lh(&dir, up);
        let translation = &rotation * (-eye);
        let translation = Translation3::from(translation);
        let transform = TransformComponent::with_translation_and_rotation(translation, rotation);
        Self {
            transform,
            fov_y_rad,
            z_near,
            z_far,
        }
    }
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        let view = self.transform.to_isometry3().to_homogeneous();
        let proj = nalgebra_glm::perspective_fov_lh_zo(
            self.fov_y_rad,
            viewport.width.get() as f32,
            viewport.height.get() as f32,
            self.z_near,
            self.z_far,
        );
        proj * view
    }
}
