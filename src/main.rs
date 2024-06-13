use bevy::{prelude::*, window::close_on_esc};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use ray_tracer::{Line, LineKind};

mod ray_tracer;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(ray_tracer::RayTracerPlugin)
        .add_systems(Update, close_on_esc)
        .add_systems(Startup, setup)
        .add_systems(Update, move_player)
        .run()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn(ray_tracer::ShaderInputs {
        player: Vec2::new(0.0, 0.0),
    });

    commands.spawn(Line {
        a: Vec2::new(-100.0, 100.0),
        b: Vec2::new(-150.0, 200.0),
        kind: LineKind::Solid,
    });
    commands.spawn(Line {
        a: Vec2::new(100.0, 100.0),
        b: Vec2::new(150.0, 200.0),
        kind: LineKind::Mirror(Color::RED),
    });
}

fn move_player(
    time: Res<Time>,
    inputs: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut ray_tracer::ShaderInputs>,
) {
    let mut shader_inputs = query.single_mut();

    let w: f32 = inputs.pressed(KeyCode::KeyW).into();
    let s: f32 = inputs.pressed(KeyCode::KeyS).into();
    let a: f32 = inputs.pressed(KeyCode::KeyA).into();
    let d: f32 = inputs.pressed(KeyCode::KeyD).into();

    let dir = Vec2::new(d - a, s - w);
    if dir.length() == 0.0 {
        return;
    }

    shader_inputs.player +=
        time.delta_seconds() * dir.normalize() * 200.0;
}
