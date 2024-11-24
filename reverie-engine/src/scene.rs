//! シーンに関するモジュール
use std::time::{Duration, Instant};

use anyhow::Context;
use nalgebra::{Affine3, Isometry3, Matrix4, Point3, Scale3, Translation3, UnitQuaternion};
use tracing_unwrap::ResultExt;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, TouchPhase},
};

use crate::{
    texture::TextureId,
    wgpu_layer::{UvVertex, VertexIndexBuffer, WgpuResource},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// [`Scene`]に登録したエンティティのインデックス
pub struct EntityIndex(pub hecs::Entity);

#[derive(Default)]
/// シーン内には複数のエンティティが存在する。
pub struct Scene {
    pub(crate) world: hecs::World,
    systems: Vec<Box<dyn System>>,
}

impl Scene {
    pub fn new_entity(
        &mut self,
        transform: TransformComponent,
        sprite: SpriteComponent,
    ) -> EntityIndex {
        let entity = self.world.spawn((transform, sprite));
        EntityIndex(entity)
    }

    pub fn attach_component<C: hecs::Component + 'static>(
        &mut self,
        entity: EntityIndex,
        component: C,
    ) {
        self.world.insert_one(entity.0, component).unwrap_or_log();
    }

    pub fn register_system<S: System + 'static>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }

    pub fn setup(&mut self, resource: &WgpuResource<'_>) {
        for (_, sprite) in self.world.query_mut::<&mut SpriteComponent>() {
            sprite.setup(resource)
        }

        for system in &mut self.systems {
            system.setup(resource);
        }
    }

    pub fn update(&mut self, frame: &Frame<'_>, resource: &WgpuResource<'_>) {
        for system in &mut self.systems {
            system.update(frame, &mut self.world, resource);
        }
    }

    pub fn render(&mut self, rp: &mut wgpu::RenderPass<'_>, resource: &WgpuResource<'_>) {
        for (_, (transform, sprite)) in self
            .world
            .query_mut::<(&TransformComponent, &mut SpriteComponent)>()
        {
            sprite.render(rp, resource, transform);
        }
    }
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("#world", &self.world.len())
            .finish()
    }
}

#[derive(Debug)]
/// エンティティの位置、回転、拡大縮小を表すコンポーネント
pub struct TransformComponent {
    pub translation: Translation3<f32>,
    pub rotation: UnitQuaternion<f32>,
    pub scale: Scale3<f32>,
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self {
            translation: Translation3::identity(),
            rotation: UnitQuaternion::identity(),
            scale: Scale3::identity(),
        }
    }
}

impl TransformComponent {
    pub fn new(
        translation: Translation3<f32>,
        scale: Scale3<f32>,
        rotation: UnitQuaternion<f32>,
    ) -> Self {
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn with_translation(translation: Translation3<f32>) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    pub fn with_translation_and_scale(translation: Translation3<f32>, scale: Scale3<f32>) -> Self {
        Self {
            translation,
            scale,
            ..Default::default()
        }
    }

    pub fn with_translation_and_rotation(
        translation: Translation3<f32>,
        rotation: UnitQuaternion<f32>,
    ) -> Self {
        Self {
            translation,
            rotation,
            ..Default::default()
        }
    }

    pub fn to_affine3(&self) -> Affine3<f32> {
        Affine3::from_matrix_unchecked(
            self.to_isometry3().to_homogeneous()
                * Matrix4::new_nonuniform_scaling(&self.scale.vector),
        )
    }

    pub fn to_isometry3(&self) -> Isometry3<f32> {
        Isometry3::from_parts(self.translation, self.rotation)
    }
}

#[derive(Debug)]
/// エンティティの見た目を表すコンポーネント
pub struct SpriteComponent {
    texture: TextureId,
    buffer: Option<VertexIndexBuffer>,
}

impl SpriteComponent {
    pub const fn new(texture: TextureId) -> Self {
        Self {
            texture,
            buffer: None,
        }
    }

    fn setup(&mut self, resource: &WgpuResource<'_>) {
        let buffer = VertexIndexBuffer::new(&resource.device, 4, 6, None).unwrap_or_log();
        self.buffer = Some(buffer);
    }

    fn render(
        &mut self,
        rp: &mut wgpu::RenderPass<'_>,
        resource: &WgpuResource<'_>,
        transform: &TransformComponent,
    ) {
        if let Some(buffer) = &mut self.buffer {
            // バッファのアップデート
            {
                let mut update = buffer.start_update(&resource.queue);
                let (min_u, min_v, max_u, max_v) = resource
                    .texture_registry
                    .get_uv(self.texture)
                    .unwrap_or_log();
                let affine = transform.to_affine3();
                const POINTS: Matrix4<f32> = Matrix4::new(
                    -0.5, 0.5, -0.5, 0.5, //
                    -0.5, -0.5, 0.5, 0.5, //
                    0.0, 0.0, 0.0, 0.0, //
                    1.0, 1.0, 1.0, 1.0, //
                );
                let points = affine.matrix() * POINTS;
                let top_left = Point3::from_homogeneous(points.column(0).into()).unwrap();
                let top_right = Point3::from_homogeneous(points.column(1).into()).unwrap();
                let bottom_left = Point3::from_homogeneous(points.column(2).into()).unwrap();
                let bottom_right = Point3::from_homogeneous(points.column(3).into()).unwrap();

                let range = {
                    let v = update.vertex_mut();
                    v.clear();
                    v.push(UvVertex {
                        position: top_left.into(),
                        uv: [min_u, min_v],
                    });
                    v.push(UvVertex {
                        position: top_right.into(),
                        uv: [max_u, min_v],
                    });
                    v.push(UvVertex {
                        position: bottom_left.into(),
                        uv: [min_u, max_v],
                    });
                    v.push(UvVertex {
                        position: bottom_right.into(),
                        uv: [max_u, max_v],
                    });
                    0..v.len()
                };
                update.set_vertex_update(range);

                let range = {
                    let i = update.index_mut();
                    i.clear();
                    i.extend_from_slice(&[0, 3, 1, 0, 2, 3]);
                    0..i.len()
                };
                update.set_index_update(range.clone());
                update.set_render_range(range.start as u32..range.end as u32);
            }

            let bind_group = resource
                .get_texture_bind_group(self.texture)
                .context("texture not found for index")
                .unwrap_or_log();
            rp.set_bind_group(0, bind_group, &[]);
            rp.set_index_buffer(buffer.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rp.set_vertex_buffer(0, buffer.vertex_buffer.slice(..));
            rp.draw_indexed(buffer.index_buffer_range.clone(), 0, 0..1);
        } else {
            tracing::warn!("buffer is not initialized");
        }
    }
}

#[derive(Debug)]
/// フレームごとに更新される情報
pub struct Frame<'a> {
    pub now: Instant,
    pub delta_time: Duration,
    pub key_events: &'a [KeyEvent],
    pub mouse_clicks: &'a [(ElementState, MouseButton, PhysicalPosition<f64>)],
    pub mouse_wheels: &'a [(MouseScrollDelta, TouchPhase, PhysicalPosition<f64>)],
    pub mouse_position: PhysicalPosition<f64>,
}

pub trait System {
    fn setup(&mut self, resource: &WgpuResource<'_>);

    fn update(&mut self, frame: &Frame<'_>, world: &mut hecs::World, resource: &WgpuResource<'_>);
}
