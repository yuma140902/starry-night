use anyhow::Context;
use nalgebra::{Matrix4, Point3};
use tracing_unwrap::ResultExt;

use crate::{
    scene::TransformComponent,
    texture::TextureId,
    wgpu_wrapper::{buffer::VertexIndexBuffer, vertex::UvVertex, WgpuResource},
};

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

    pub(crate) fn setup(&mut self, resource: &WgpuResource<'_>) {
        let buffer = VertexIndexBuffer::new(&resource.device, 4, 6, None).unwrap_or_log();
        self.buffer = Some(buffer);
    }

    pub(crate) fn render(
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
                    0.5, 0.5, -0.5, -0.5, //
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
