use bevy::prelude::*;

pub const BG_COLOR: Color = Color::srgb(0.08, 0.08, 0.12);
pub const ACCENT: Color = Color::srgb(0.4, 0.6, 1.0);
pub const TEXT_PRIMARY: Color = Color::srgb(0.95, 0.95, 0.97);
pub const TEXT_SECONDARY: Color = Color::srgb(0.6, 0.6, 0.65);
pub const TEXT_DIM: Color = Color::srgb(0.5, 0.5, 0.55);

pub fn spawn_label(parent: &mut ChildSpawnerCommands, text: impl Into<String>, size: f32, color: Color) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    ));
}
