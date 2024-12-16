use nalgebra::{Scale3, Translation3, UnitQuaternion, UnitVector3, Vector3};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use reverie_engine::{
    scene::{EntityIndex, Frame, Scene, SpriteComponent, System, TransformComponent},
    wgpu_wrapper::WgpuResource,
    Game,
};
use winit::event::{ElementState, MouseButton};

fn setup_cli() {
    use tracing_subscriber::fmt::format::FmtSpan;
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_thread_ids(true)
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

fn main() -> anyhow::Result<()> {
    setup_cli();

    let game = LineDefense::default();
    reverie_engine::start_engine(game)
}

#[derive(Debug, Default)]
pub struct LineDefense {}

impl Game for LineDefense {
    fn generate_scene(
        &mut self,
        registry: &mut reverie_engine::texture::TextureRegistry,
    ) -> anyhow::Result<Scene> {
        let tex_uv = registry
            .new_texture(
                image::load_from_memory(include_bytes!("../assets/uv_tp.png"))
                    .unwrap()
                    .to_rgba8(),
                None,
            )
            .into();

        let apple_and_cat = registry.create_altas_texture(256, 512, None);

        let tex_apple = registry
            .allocate_sub_image(
                apple_and_cat,
                image::load_from_memory(include_bytes!("../assets/apple.png"))
                    .unwrap()
                    .to_rgba8(),
            )
            .unwrap()
            .into();
        let tex_cat = registry
            .allocate_sub_image(
                apple_and_cat,
                image::load_from_memory(include_bytes!("../assets/cat.png"))
                    .unwrap()
                    .to_rgba8(),
            )
            .unwrap()
            .into();

        let mut scene = Scene::default();

        let mut rng = SmallRng::seed_from_u64(42);

        let mut characters = Vec::new();
        for i in 0..10 {
            for j in 0..9 {
                let x = 0.1_f32.mul_add(i as f32, -0.4);
                let y = 0.1_f32.mul_add(j as f32, -0.4);
                let character = scene.new_entity(
                    TransformComponent::new(
                        Translation3::new(x, y, 0.0),
                        Scale3::new(0.07, 0.07, 1.0),
                        UnitQuaternion::from_axis_angle(
                            &UnitVector3::new_normalize(Vector3::new(
                                rng.gen_range(-1.0..1.0),
                                rng.gen_range(-1.0..1.0),
                                rng.gen_range(-1.0..1.0),
                            )),
                            rng.gen_range(0.0..80.0_f32).to_radians(),
                        ),
                    ),
                    SpriteComponent::new(if rng.gen_bool(0.5) {
                        tex_apple
                    } else {
                        tex_cat
                    }),
                );
                characters.push(character);
            }
        }

        scene.new_entity(
            TransformComponent::with_translation_and_scale(
                Translation3::new(0.0, 0.0, 0.0),
                Scale3::new(1.0, 1.0, 1.0),
            ),
            SpriteComponent::new(tex_uv),
        );
        scene.register_system(RotateCharacters::with_characters(&characters));

        Ok(scene)
    }
}

#[derive(Debug)]
struct RotateCharacters {
    is_moving: bool,
    ids: Vec<EntityIndex>,
}

impl RotateCharacters {
    pub fn with_characters(ids: &[EntityIndex]) -> Self {
        Self {
            is_moving: false,
            ids: Vec::from(ids),
        }
    }
}

impl System for RotateCharacters {
    fn setup(&mut self, _resource: &WgpuResource) {}

    fn update(&mut self, frame: &Frame, world: &mut hecs::World, _resource: &WgpuResource) {
        for e in world.iter() {
            if self.ids.iter().any(|i| i.0 == e.entity()) {
                if let Some(ref mut transform) = e.get::<&mut TransformComponent>() {
                    if self.is_moving {
                        let rot = UnitQuaternion::from_axis_angle(
                            &Vector3::z_axis(),
                            10.0 * frame.delta_time.as_secs_f32(),
                        );
                        transform.rotation *= rot;
                    }
                }
            }
        }
        for (state, button, _) in frame.mouse_clicks {
            match (state, button) {
                (ElementState::Pressed, MouseButton::Left) => self.is_moving = true,
                (ElementState::Released, MouseButton::Left) => self.is_moving = false,
                _ => {}
            }
        }
    }
}
