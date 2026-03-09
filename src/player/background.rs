use bevy::{
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{Material2d, Material2dPlugin},
};

use crate::states::AppState;

const SHADER_COUNT: usize = 5;

macro_rules! define_time_material {
    ($name:ident, $shader:literal) => {
        #[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
        pub struct $name {
            #[uniform(0)]
            pub time: f32,
        }

        impl Material2d for $name {
            fn fragment_shader() -> ShaderRef {
                $shader.into()
            }
        }
    };
}

define_time_material!(PlasmaMaterial, "shaders/plasma.wgsl");
define_time_material!(AuroraMaterial, "shaders/aurora.wgsl");
define_time_material!(WavesMaterial, "shaders/waves.wgsl");
define_time_material!(NebulaMaterial, "shaders/nebula.wgsl");
define_time_material!(StarfieldMaterial, "shaders/starfield.wgsl");

pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            Material2dPlugin::<PlasmaMaterial>::default(),
            Material2dPlugin::<AuroraMaterial>::default(),
            Material2dPlugin::<WavesMaterial>::default(),
            Material2dPlugin::<NebulaMaterial>::default(),
            Material2dPlugin::<StarfieldMaterial>::default(),
        ))
        .init_resource::<ActiveTheme>()
        .add_systems(
            Update,
            tick_material_time
                .run_if(in_state(AppState::Playing))
                .run_if(super::no_player_overlay),
        );
    }
}

#[derive(Resource)]
pub struct ActiveTheme {
    pub index: usize,
}

impl Default for ActiveTheme {
    fn default() -> Self {
        Self { index: 0 }
    }
}

impl ActiveTheme {
    pub fn theme_count() -> usize {
        SHADER_COUNT + 1
    }

    pub fn is_video(&self) -> bool {
        self.index % Self::theme_count() >= SHADER_COUNT
    }

    pub fn name(&self) -> &str {
        match self.index % Self::theme_count() {
            0 => "Plasma",
            1 => "Aurora",
            2 => "Waves",
            3 => "Nebula",
            4 => "Starfield",
            5 => "Video",
            _ => unreachable!(),
        }
    }

    pub fn next(&mut self) {
        self.index = (self.index + 1) % Self::theme_count();
    }
}

#[derive(Component)]
pub struct BackgroundQuad;

#[derive(Component)]
pub struct BackgroundMaterialHandle(pub MaterialVariant);

pub enum MaterialVariant {
    Plasma(Handle<PlasmaMaterial>),
    Aurora(Handle<AuroraMaterial>),
    Waves(Handle<WavesMaterial>),
    Nebula(Handle<NebulaMaterial>),
    Starfield(Handle<StarfieldMaterial>),
}

macro_rules! spawn_theme_variant {
    ($commands:expr, $mesh:expr, $transform:expr, $materials:expr, $mat_type:ident, $variant:ident) => {{
        let handle = $materials.add($mat_type { time: 0.0 });
        $commands.spawn((
            BackgroundQuad,
            BackgroundMaterialHandle(MaterialVariant::$variant(handle.clone())),
            Mesh2d($mesh),
            MeshMaterial2d(handle),
            $transform,
        ));
    }};
}

pub fn spawn_background(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    plasma_materials: &mut ResMut<Assets<PlasmaMaterial>>,
    aurora_materials: &mut ResMut<Assets<AuroraMaterial>>,
    waves_materials: &mut ResMut<Assets<WavesMaterial>>,
    nebula_materials: &mut ResMut<Assets<NebulaMaterial>>,
    starfield_materials: &mut ResMut<Assets<StarfieldMaterial>>,
    theme: &ActiveTheme,
) {
    if theme.is_video() {
        return;
    }

    let mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let transform = Transform::from_scale(Vec3::new(3000.0, 2000.0, 1.0))
        .with_translation(Vec3::new(0.0, 0.0, -10.0));

    match theme.index % ActiveTheme::theme_count() {
        1 => spawn_theme_variant!(commands, mesh, transform, aurora_materials, AuroraMaterial, Aurora),
        2 => spawn_theme_variant!(commands, mesh, transform, waves_materials, WavesMaterial, Waves),
        3 => spawn_theme_variant!(commands, mesh, transform, nebula_materials, NebulaMaterial, Nebula),
        4 => spawn_theme_variant!(commands, mesh, transform, starfield_materials, StarfieldMaterial, Starfield),
        _ => spawn_theme_variant!(commands, mesh, transform, plasma_materials, PlasmaMaterial, Plasma),
    }
}

pub fn despawn_background(commands: &mut Commands, query: &Query<Entity, With<BackgroundQuad>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
}

fn tick_material_time(
    time: Res<Time>,
    bg_query: Query<&BackgroundMaterialHandle, With<BackgroundQuad>>,
    mut plasma: ResMut<Assets<PlasmaMaterial>>,
    mut aurora: ResMut<Assets<AuroraMaterial>>,
    mut waves: ResMut<Assets<WavesMaterial>>,
    mut nebula: ResMut<Assets<NebulaMaterial>>,
    mut starfield: ResMut<Assets<StarfieldMaterial>>,
) {
    let t = time.elapsed_secs();
    for holder in &bg_query {
        match &holder.0 {
            MaterialVariant::Plasma(h) => { if let Some(m) = plasma.get_mut(h) { m.time = t; } }
            MaterialVariant::Aurora(h) => { if let Some(m) = aurora.get_mut(h) { m.time = t; } }
            MaterialVariant::Waves(h) => { if let Some(m) = waves.get_mut(h) { m.time = t; } }
            MaterialVariant::Nebula(h) => { if let Some(m) = nebula.get_mut(h) { m.time = t; } }
            MaterialVariant::Starfield(h) => { if let Some(m) = starfield.get_mut(h) { m.time = t; } }
        }
    }
}
