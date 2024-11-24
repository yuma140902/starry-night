use nalgebra::{Scale3, Translation3, Unit, UnitQuaternion, Vector3};
use reverie_engine::{
    game::Game,
    scene::{EntityIndex, Frame, Scene, SpriteComponent, System, TransformComponent},
    wgpu_layer::WgpuResource,
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
        let tex_apple = registry
            .new_texture(
                image::load_from_memory(include_bytes!("../assets/apple.png"))
                    .unwrap()
                    .to_rgba8(),
                None,
            )
            .into();

        let cat_and_snake = registry.create_altas_texture(256, 512, None);

        let tex_cat = registry
            .allocate_sub_image(
                cat_and_snake,
                image::load_from_memory(include_bytes!("../assets/cat.png"))
                    .unwrap()
                    .to_rgba8(),
            )
            .unwrap()
            .into();
        let tex_snake = registry
            .allocate_sub_image(
                cat_and_snake,
                image::load_from_memory(include_bytes!("../assets/snake.png"))
                    .unwrap()
                    .to_rgba8(),
            )
            .unwrap()
            .into();

        let mut scene = Scene::default();

        for i in 0..10 {
            for j in 0..9 {
                let x = 50.0_f32.mul_add(i as f32, -225.0);
                let y = 50.0_f32.mul_add(j as f32, -200.0);
                scene.new_entity(
                    TransformComponent::with_translation_and_scale(
                        Translation3::new(x, y, 0.0),
                        Scale3::new(30.0, 30.0, 1.0),
                    ),
                    SpriteComponent::new(tex_apple),
                );
            }
        }

        scene.new_entity(
            TransformComponent::with_translation_and_scale(
                Translation3::new(0.0, 0.0, 0.0),
                Scale3::new(100.0, 100.0, 1.0),
            ),
            SpriteComponent::new(tex_cat),
        );
        let snake = scene.new_entity(
            TransformComponent::with_translation_and_scale(
                Translation3::new(-50.0, -50.0, 0.0),
                Scale3::new(100.0, 100.0, 1.0),
            ),
            SpriteComponent::new(tex_snake),
        );
        scene.register_system(PlayerController::with_player(snake));

        Ok(scene)
    }
}

#[derive(Debug)]
struct PlayerController {
    is_moving: bool,
    id: EntityIndex,
}

impl PlayerController {
    pub const fn with_player(id: EntityIndex) -> Self {
        Self {
            is_moving: false,
            id,
        }
    }
}

impl System for PlayerController {
    fn setup(&mut self, _resource: &WgpuResource) {}

    fn update(&mut self, frame: &Frame, world: &mut hecs::World, _resource: &WgpuResource) {
        if let Ok(ref mut transform) = world.get::<&mut TransformComponent>(self.id.0) {
            if self.is_moving {
                transform.translation.x = frame.mouse_position.x as f32;
                transform.translation.y = frame.mouse_position.y as f32;
                transform.rotation = UnitQuaternion::from_axis_angle(
                    &Unit::new_normalize(Vector3::z()),
                    -10.0 * frame.delta_time.as_secs_f32(),
                ) * transform.rotation;
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
}
