//! wgpu をラップするモジュール
use std::{borrow::Cow, num::NonZeroU32, ops::Range};

use anyhow::Context;
use nalgebra::{Matrix4, Point3, Scale3, Translation3, UnitQuaternion, UnitVector3, Vector3};
use wgpu::{self as w, util::DeviceExt};

use crate::{
    scene::{Scene, TransformComponent},
    texture::{TextureId, TextureRegistry},
};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
/// UV 座標を持つ頂点
///
/// * `position`: 頂点の位置
/// * `uv`: UV 座標
pub struct UvVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

impl UvVertex {
    pub const fn desc() -> w::VertexBufferLayout<'static> {
        w::VertexBufferLayout {
            array_stride: size_of::<Self>() as w::BufferAddress,
            step_mode: w::VertexStepMode::Vertex,
            attributes: &[
                w::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: w::VertexFormat::Float32x3,
                },
                w::VertexAttribute {
                    offset: size_of::<[f32; 3]>() as w::BufferAddress,
                    shader_location: 1,
                    format: w::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// wgpu を使うためのリソースをまとめた構造体
pub struct WgpuResource<'window> {
    pub transform_uniform_buffer: w::Buffer,
    pub texture_bind_group_layout: w::BindGroupLayout,
    pub texture_sampler: w::Sampler,
    pub uniform_bind_group: w::BindGroup,
    pub render_pipeline: w::RenderPipeline,
    pub surface: w::Surface<'window>,
    pub surface_config: w::SurfaceConfiguration,
    pub device: w::Device,
    pub queue: w::Queue,
    pub texture_registry: TextureRegistry,
    pub camera: Camera,
    pub viewport: Viewport,
}

impl<'window> WgpuResource<'window> {
    pub const TEXTURE_BINDING: u32 = 0;
    pub const SAMPLER_BINDING: u32 = 1;

    /// 初期化する
    ///
    /// * `surface_target`: 描画対象の surface
    /// * `width`: surface の幅
    /// * `height`: surface の高さ
    /// * `packed_image1`: テクスチャ
    /// * `vertex_buffer_max_elements`: 頂点バッファの最大要素数
    /// * `index_buffer_max_elements`: インデックスバッファの最大要素数
    pub async fn setup<S>(
        surface_target: S,
        width: NonZeroU32,
        height: NonZeroU32,
    ) -> anyhow::Result<Self>
    where
        S: Into<w::SurfaceTarget<'window>> + Send,
    {
        let (_instance, surface, surface_format, surface_config, _adapter, device, queue) =
            setup_instance_surface_adapter_device_queue(
                surface_target,
                width.into(),
                height.into(),
            )
            .await?;
        tracing::trace!(
            ?surface,
            ?surface_format,
            ?surface_config,
            ?device,
            ?queue,
            "setup_instance_surface_adapter_device_queue"
        );

        let shader = setup_shader(&device)?;
        tracing::trace!(?shader, "setup_shader");

        let camera: Camera = CameraOrthographic {
            transform: TransformComponent::with_translation_and_rotation(
                Translation3::new(100.0, 0.0, 0.0),
                UnitQuaternion::from_axis_angle(
                    &UnitVector3::new_normalize(Vector3::new(0.0, 0.0, 1.0)),
                    30.0_f32.to_radians(),
                ),
            ),
            size: 300.0,
        }
        .into();
        let viewport = Viewport { width, height };

        let transform_uniform_buffer = setup_uniform_buffer(&device, &camera, &viewport)?;

        let sampler = setup_sampler(&device)?;
        tracing::trace!(?sampler, "setup_sampler");

        let (uniform_bind_group_layout, uniform_bind_group) =
            setup_uniform_bind_group(&transform_uniform_buffer, &device)?;
        tracing::trace!(
            ?uniform_bind_group_layout,
            ?uniform_bind_group,
            "setup_uniform_bind_group"
        );

        let texture_bind_group_layout = WgpuTexture::bind_group_layout(
            &device,
            Some("texture bind group layout"),
            Self::TEXTURE_BINDING,
            Self::SAMPLER_BINDING,
        );
        tracing::trace!(
            ?texture_bind_group_layout,
            "setup_texture_bind_group_layout"
        );

        let render_pipeline = setup_render_pipeline(
            &shader,
            &[&texture_bind_group_layout, &uniform_bind_group_layout],
            surface_format,
            &device,
        )?;
        tracing::trace!(?render_pipeline, "setup_render_pipeline");

        let texture_registry = TextureRegistry::default();
        tracing::trace!(?texture_registry, "setup_texture_registry");

        Ok(Self {
            transform_uniform_buffer,
            texture_bind_group_layout,
            texture_sampler: sampler,
            uniform_bind_group,
            render_pipeline,
            surface,
            surface_config,
            device,
            queue,
            texture_registry,
            camera,
            viewport,
        })
    }

    pub fn resize(&mut self, width: NonZeroU32, height: NonZeroU32) {
        self.surface_config.width = width.get();
        self.surface_config.height = height.get();
        self.surface.configure(&self.device, &self.surface_config);

        self.viewport.width = width;
        self.viewport.height = height;

        let matrix = self
            .camera
            .get_matrix_world_to_render_coordinate(&self.viewport);
        self.queue.write_buffer(
            &self.transform_uniform_buffer,
            0,
            bytemuck::cast_slice(matrix.as_slice()),
        );
    }

    pub fn get_texture_bind_group(&self, texture: TextureId) -> anyhow::Result<&w::BindGroup> {
        self.texture_registry.get_bind_group(texture)
    }

    pub fn render(&self, scene: &mut Scene) {
        if let Ok(surface_texture) = self.surface.get_current_texture() {
            let output = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Main CommandEncoder"),
                });
            {
                let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SpriteComponent Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &output,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 54.0 / 255.0,
                                g: 77.0 / 255.0,
                                b: 118.0 / 255.0,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                rp.set_pipeline(&self.render_pipeline);
                rp.set_bind_group(1, &self.uniform_bind_group, &[]);

                scene.render(&mut rp, self);
            }
            self.queue.submit(Some(encoder.finish()));

            surface_texture.present();
        } else {
            tracing::warn!("no surface texture");
        }
    }
}

#[tracing::instrument(level = "trace", skip(surface_target))]
async fn setup_instance_surface_adapter_device_queue<'window, S>(
    surface_target: S,
    width: u32,
    height: u32,
) -> anyhow::Result<(
    w::Instance,
    w::Surface<'window>,
    w::TextureFormat,
    w::SurfaceConfiguration,
    w::Adapter,
    w::Device,
    w::Queue,
)>
where
    S: Into<w::SurfaceTarget<'window>> + Send,
{
    let instance = w::Instance::default();

    let surface = instance
        .create_surface(surface_target)
        .context("fail: create surface")?;
    tracing::trace!(?surface, "created surface");

    let adapter = instance
        .request_adapter(&w::RequestAdapterOptions {
            power_preference: w::PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .context("fail: request adapter")?;
    tracing::trace!(?adapter, "requested adapter");

    let (device, queue) = adapter
        .request_device(
            &w::DeviceDescriptor {
                label: Some("Main Device"),
                required_features: w::Features::empty(),
                required_limits: w::Limits::default(),
                memory_hints: w::MemoryHints::default(),
            },
            None,
        )
        .await
        .context("fail: request device")?;
    tracing::trace!(?device, ?queue, "requested device and queue");

    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|f| f.is_srgb())
        .copied()
        .or_else(|| surface_caps.formats.first().copied())
        .context("fail: no surface format which supports SRGB")?;
    let config = w::SurfaceConfiguration {
        usage: w::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    Ok((
        instance,
        surface,
        surface_format,
        config,
        adapter,
        device,
        queue,
    ))
}

fn setup_shader(device: &w::Device) -> anyhow::Result<w::ShaderModule> {
    Ok(device.create_shader_module(w::ShaderModuleDescriptor {
        label: Some("Shader from shader.wgsl"),
        source: w::ShaderSource::Wgsl(Cow::Borrowed(include_str!("./shader.wgsl"))),
    }))
}

fn setup_uniform_buffer(
    device: &w::Device,
    camera: &Camera,
    viewport: &Viewport,
) -> anyhow::Result<w::Buffer> {
    let initial_matrix = camera.get_matrix_world_to_render_coordinate(viewport);
    Ok(device.create_buffer_init(&w::util::BufferInitDescriptor {
        label: Some("Pixel to Render Coordinate Matrix Buffer"),
        contents: bytemuck::cast_slice(initial_matrix.as_slice()),
        usage: w::BufferUsages::UNIFORM | w::BufferUsages::COPY_DST,
    }))
}

fn setup_sampler(device: &w::Device) -> anyhow::Result<w::Sampler> {
    Ok(device.create_sampler(&w::SamplerDescriptor {
        label: Some("Main Texture Sampler"),
        address_mode_u: w::AddressMode::ClampToEdge,
        address_mode_v: w::AddressMode::ClampToEdge,
        address_mode_w: w::AddressMode::ClampToEdge,
        mag_filter: w::FilterMode::Nearest,
        min_filter: w::FilterMode::Nearest,
        mipmap_filter: w::FilterMode::Nearest,
        ..Default::default()
    }))
}

fn setup_uniform_bind_group(
    transform_uniform_buffer: &w::Buffer,
    device: &w::Device,
) -> anyhow::Result<(w::BindGroupLayout, w::BindGroup)> {
    let bind_group_layout = device.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
        label: Some("Main Bind Group Layout"),
        entries: &[w::BindGroupLayoutEntry {
            binding: 0,
            visibility: w::ShaderStages::VERTEX,
            ty: w::BindingType::Buffer {
                ty: w::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: w::BufferSize::new(size_of::<[f32; 4 * 4]>() as u64),
            },
            count: None,
        }],
    });

    let bind_group = device.create_bind_group(&w::BindGroupDescriptor {
        label: Some("Main Bind Group"),
        layout: &bind_group_layout,
        entries: &[w::BindGroupEntry {
            binding: 0,
            resource: transform_uniform_buffer.as_entire_binding(),
        }],
    });

    Ok((bind_group_layout, bind_group))
}

fn setup_render_pipeline(
    shader: &w::ShaderModule,
    bind_group_layouts: &[&w::BindGroupLayout],
    surface_format: w::TextureFormat,
    device: &w::Device,
) -> anyhow::Result<w::RenderPipeline> {
    let render_pipeline_layout = device.create_pipeline_layout(&w::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts,
        push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&w::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: w::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[UvVertex::desc()],
        },
        fragment: Some(w::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(w::ColorTargetState {
                format: surface_format,
                blend: Some(w::BlendState {
                    color: w::BlendComponent {
                        src_factor: w::BlendFactor::SrcAlpha,
                        dst_factor: w::BlendFactor::OneMinusSrcAlpha,
                        operation: w::BlendOperation::Add,
                    },
                    alpha: w::BlendComponent::OVER,
                }),
                write_mask: w::ColorWrites::ALL,
            })],
        }),
        primitive: w::PrimitiveState {
            topology: w::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: w::FrontFace::Ccw,
            cull_mode: Some(w::Face::Back),
            unclipped_depth: false,
            polygon_mode: w::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: w::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    Ok(render_pipeline)
}

#[derive(Debug)]
/// 頂点バッファとインデックスバッファをまとめた構造体
pub(crate) struct VertexIndexBuffer {
    pub(crate) vertex_buffer: w::Buffer,
    vertex_array: Vec<UvVertex>,
    pub(crate) index_buffer: w::Buffer,
    index_array: Vec<u16>,
    pub(crate) index_buffer_range: Range<u32>,
}

impl VertexIndexBuffer {
    pub fn new(
        device: &w::Device,
        max_vertices: usize,
        max_indices: usize,
        label: Option<&str>,
    ) -> anyhow::Result<Self> {
        let name_v = label.map(|label| format!("{label} [vertex part]"));
        let name_i = label.map(|label| format!("{label} [index part]"));

        let vertex_buffer = device.create_buffer(&w::BufferDescriptor {
            label: name_v.as_deref(),
            usage: w::BufferUsages::VERTEX | w::BufferUsages::COPY_DST,
            size: (max_vertices * size_of::<UvVertex>()) as u64,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&w::BufferDescriptor {
            label: name_i.as_deref(),
            usage: w::BufferUsages::INDEX | w::BufferUsages::COPY_DST,
            size: (max_indices * size_of::<u16>()) as u64,
            mapped_at_creation: false,
        });

        Ok(Self {
            vertex_buffer,
            vertex_array: Vec::with_capacity(max_vertices),
            index_buffer,
            index_array: Vec::with_capacity(max_indices),
            index_buffer_range: 0..0,
        })
    }

    pub fn start_update<'a>(&'a mut self, queue: &'a w::Queue) -> VertexIndexBufferUpdater<'a> {
        VertexIndexBufferUpdater {
            buffer: self,
            queue,
            vertex_update: 0..0,
            index_update: 0..0,
        }
    }

    fn send_to_gpu(
        &self,
        queue: &w::Queue,
        vertex_update: Range<usize>,
        index_update: Range<usize>,
    ) {
        if !vertex_update.is_empty() {
            queue.write_buffer(
                &self.vertex_buffer,
                vertex_update.start as u64,
                bytemuck::cast_slice(&self.vertex_array[vertex_update]),
            );
        }
        if !index_update.is_empty() {
            queue.write_buffer(
                &self.index_buffer,
                index_update.start as u64,
                bytemuck::cast_slice::<u16, u8>(&self.index_array[index_update]),
            );
        }
    }
}

#[derive(Debug)]
/// [`VertexIndexBuffer`] の更新を行うための構造体。Drop されると GPU にデータを送信する
pub(crate) struct VertexIndexBufferUpdater<'a> {
    buffer: &'a mut VertexIndexBuffer,
    queue: &'a w::Queue,
    vertex_update: Range<usize>,
    index_update: Range<usize>,
}

impl<'a> VertexIndexBufferUpdater<'a> {
    pub fn vertex_mut(&mut self) -> &mut Vec<UvVertex> {
        &mut self.buffer.vertex_array
    }

    pub fn index_mut(&mut self) -> &mut Vec<u16> {
        &mut self.buffer.index_array
    }

    /// 更新した頂点バッファの範囲を設定する
    pub fn set_vertex_update(&mut self, range: Range<usize>) {
        self.vertex_update = range;
    }

    /// 更新したインデックスバッファの範囲を設定する
    pub fn set_index_update(&mut self, range: Range<usize>) {
        self.index_update = range;
    }

    /// インデックスバッファの描画範囲を設定する
    pub fn set_render_range(&mut self, range: Range<u32>) {
        self.buffer.index_buffer_range = range;
    }
}

impl<'a> std::ops::Drop for VertexIndexBufferUpdater<'a> {
    fn drop(&mut self) {
        self.buffer.send_to_gpu(
            self.queue,
            self.vertex_update.clone(),
            self.index_update.clone(),
        )
    }
}

#[derive(Debug)]
/// wgpu で使うテクスチャ
pub(crate) struct WgpuTexture {
    pub(crate) texture: w::Texture,
    pub(crate) view: w::TextureView,
}

impl WgpuTexture {
    pub fn from_image(
        device: &w::Device,
        queue: &w::Queue,
        image: &image::RgbaImage,
        label: Option<&str>,
    ) -> Self {
        let (width, height) = image.dimensions();

        let size = w::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            texture.as_image_copy(),
            image,
            w::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&Default::default());

        Self { texture, view }
    }

    pub fn width(&self) -> u32 {
        self.texture.width()
    }

    pub fn height(&self) -> u32 {
        self.texture.height()
    }

    pub(crate) fn bind_group_layout(
        device: &w::Device,
        label: Option<&str>,
        texture_binding: u32,
        sampler_binding: u32,
    ) -> w::BindGroupLayout {
        device.create_bind_group_layout(&w::BindGroupLayoutDescriptor {
            label,
            entries: &[
                w::BindGroupLayoutEntry {
                    binding: texture_binding,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Texture {
                        sample_type: w::TextureSampleType::Float { filterable: true },
                        view_dimension: w::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                w::BindGroupLayoutEntry {
                    binding: sampler_binding,
                    visibility: w::ShaderStages::FRAGMENT,
                    ty: w::BindingType::Sampler(w::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    pub(crate) fn create_bind_group(
        &self,
        device: &w::Device,
        label: Option<&str>,
        layout: &w::BindGroupLayout,
        sampler: &w::Sampler,
        texture_binding: u32,
        sampler_binding: u32,
    ) -> w::BindGroup {
        device.create_bind_group(&w::BindGroupDescriptor {
            label,
            layout,
            entries: &[
                w::BindGroupEntry {
                    binding: texture_binding,
                    resource: w::BindingResource::TextureView(&self.view),
                },
                w::BindGroupEntry {
                    binding: sampler_binding,
                    resource: w::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

#[derive(Debug)]
pub enum Camera {
    Orthographic(CameraOrthographic),
    PixelPerfect(CameraPixelPerfect),
}

impl Camera {
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        match self {
            Self::Orthographic(camera) => camera.get_matrix_world_to_render_coordinate(viewport),
            Self::PixelPerfect(camera) => camera.get_matrix_world_to_render_coordinate(viewport),
        }
    }
}

impl From<CameraOrthographic> for Camera {
    fn from(value: CameraOrthographic) -> Self {
        Self::Orthographic(value)
    }
}

impl From<CameraPixelPerfect> for Camera {
    fn from(value: CameraPixelPerfect) -> Self {
        Self::PixelPerfect(value)
    }
}

#[derive(Debug)]
pub struct CameraOrthographic {
    /// カメラの姿勢。`scale` は無視される
    transform: TransformComponent,
    /// カメラの縦半分のサイズ (世界座標系)
    size: f32,
}

impl CameraOrthographic {
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        let vp_width = viewport.width.get() as f32;
        let vp_height = viewport.height.get() as f32;

        let height = self.size;
        let width = height * vp_width / vp_height;
        Scale3::new(1.0 / width, -1.0 / height, 1.0).to_homogeneous()
            * self.transform.to_isometry3().to_homogeneous()
    }
}

#[derive(Debug)]
pub struct CameraPixelPerfect {
    // 左上にあるピクセルが対応するワールド座標
    position: Point3<f32>,
}

impl CameraPixelPerfect {
    pub fn get_matrix_world_to_render_coordinate(&self, viewport: &Viewport) -> Matrix4<f32> {
        let width = viewport.width.get() as f32;
        let height = viewport.height.get() as f32;

        Translation3::from([-1.0, 1.0, 0.0]).to_homogeneous()
            * Scale3::new(2.0 / width, -2.0 / height, 1.0).to_homogeneous()
            * Translation3::from(-&self.position).to_homogeneous()
    }
}

#[derive(Debug)]
pub struct Viewport {
    width: NonZeroU32,
    height: NonZeroU32,
}
