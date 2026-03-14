use bevy::prelude::*;

pub mod layout {
    pub const SIDEBAR_WIDTH: f32 = 220.0;
    pub const MAIN_CONTENT_WIDTH: f32 = 700.0;
    pub const OVERLAY_WIDTH_SM: f32 = 320.0;
    pub const OVERLAY_WIDTH_MD: f32 = 340.0;
    pub const OVERLAY_WIDTH_LG: f32 = 460.0;
    pub const CARD_MIN_HEIGHT: f32 = 64.0;
    pub const ART_SIZE: f32 = 48.0;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Resource, Clone)]
pub struct UiTheme {
    pub mode: ThemeMode,
    pub bg: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub sidebar_bg: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_dim: Color,
    pub card_bg: Color,
    pub card_hover: Color,
    pub sidebar_btn: Color,
    pub sidebar_btn_hover: Color,
    pub popup_btn: Color,
    pub popup_btn_hover: Color,
    pub badge_ready: Color,
    pub badge_lyrics: Color,
    pub badge_not_analyzed: Color,
    pub badge_queued: Color,
    pub badge_analyzing: Color,
    pub badge_failed: Color,
    pub sung_color: Color,
    pub unsung_color: Color,
    pub unsung_estimated: Color,
    pub next_line_color: Color,
    pub lyric_backdrop: Color,
    pub lyric_backdrop_next: Color,
    pub countdown_color: Color,
    pub countdown_bg: Color,
    pub hud_primary: Color,
    pub hud_secondary: Color,
    pub hud_dim: Color,
    pub pitch_ref_line: Srgba,
    pub pitch_user_base: Srgba,
    pub overlay_dim: Color,
    pub spinner_overlay: Color,
    pub skip_btn_bg: Color,
    pub skip_btn_hover: Color,
    pub skip_btn_border: Color,
    pub skip_btn_text: Color,
    pub star_gold: Color,
    pub cache_videos: Color,
    pub cache_models: Color,
    pub badge_video: Color,
}

impl UiTheme {
    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            bg: Color::srgb(0.075, 0.075, 0.118),
            surface: Color::srgb(0.11, 0.11, 0.17),
            surface_hover: Color::srgb(0.16, 0.16, 0.22),
            sidebar_bg: Color::srgb(0.055, 0.055, 0.085),
            accent: Color::srgb(0.42, 0.54, 1.0),
            accent_hover: Color::srgb(0.52, 0.64, 1.0),
            text_primary: Color::srgb(0.93, 0.93, 0.96),
            text_secondary: Color::srgb(0.55, 0.55, 0.62),
            text_dim: Color::srgb(0.42, 0.42, 0.48),
            card_bg: Color::srgb(0.11, 0.11, 0.17),
            card_hover: Color::srgb(0.16, 0.16, 0.22),
            sidebar_btn: Color::srgb(0.11, 0.11, 0.17),
            sidebar_btn_hover: Color::srgb(0.18, 0.18, 0.25),
            popup_btn: Color::srgb(0.16, 0.16, 0.22),
            popup_btn_hover: Color::srgb(0.21, 0.21, 0.28),
            badge_ready: Color::srgb(0.18, 0.68, 0.28),
            badge_lyrics: Color::srgb(0.18, 0.52, 0.82),
            badge_not_analyzed: Color::srgb(0.45, 0.45, 0.50),
            badge_queued: Color::srgb(0.68, 0.53, 0.08),
            badge_analyzing: Color::srgb(0.88, 0.68, 0.08),
            badge_failed: Color::srgb(0.78, 0.18, 0.18),
            sung_color: Color::srgb(0.4, 0.75, 1.0),
            unsung_color: Color::srgba(1.0, 1.0, 1.0, 0.95),
            unsung_estimated: Color::srgba(1.0, 0.85, 0.55, 0.75),
            next_line_color: Color::srgba(1.0, 1.0, 1.0, 0.35),
            lyric_backdrop: Color::srgba(0.0, 0.0, 0.0, 0.55),
            lyric_backdrop_next: Color::srgba(0.0, 0.0, 0.0, 0.35),
            countdown_color: Color::srgb(0.4, 0.75, 1.0),
            countdown_bg: Color::srgba(0.0, 0.0, 0.0, 0.6),
            hud_primary: Color::WHITE,
            hud_secondary: Color::srgba(1.0, 1.0, 1.0, 0.6),
            hud_dim: Color::srgba(1.0, 1.0, 1.0, 0.5),
            pitch_ref_line: Srgba::new(0.5, 0.7, 1.0, 0.45),
            pitch_user_base: Srgba::new(0.85, 0.85, 1.0, 1.0),
            overlay_dim: Color::srgba(0.0, 0.0, 0.0, 0.6),
            spinner_overlay: Color::srgba(0.0, 0.0, 0.0, 0.55),
            skip_btn_bg: Color::srgba(0.0, 0.0, 0.0, 0.5),
            skip_btn_hover: Color::srgba(0.2, 0.2, 0.3, 0.7),
            skip_btn_border: Color::srgba(1.0, 1.0, 1.0, 0.4),
            skip_btn_text: Color::srgba(1.0, 1.0, 1.0, 0.8),
            star_gold: Color::srgb(1.0, 0.84, 0.0),
            cache_videos: Color::srgb(0.28, 0.72, 0.42),
            cache_models: Color::srgb(0.88, 0.62, 0.18),
            badge_video: Color::srgba(0.55, 0.65, 1.0, 0.85),
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            bg: Color::srgb(0.955, 0.955, 0.97),
            surface: Color::srgb(1.0, 1.0, 1.0),
            surface_hover: Color::srgb(0.93, 0.93, 0.96),
            sidebar_bg: Color::srgb(0.92, 0.92, 0.94),
            accent: Color::srgb(0.29, 0.42, 0.97),
            accent_hover: Color::srgb(0.36, 0.49, 1.0),
            text_primary: Color::srgb(0.12, 0.12, 0.15),
            text_secondary: Color::srgb(0.42, 0.42, 0.48),
            text_dim: Color::srgb(0.58, 0.58, 0.62),
            card_bg: Color::srgb(1.0, 1.0, 1.0),
            card_hover: Color::srgb(0.94, 0.94, 0.97),
            sidebar_btn: Color::srgb(0.96, 0.96, 0.98),
            sidebar_btn_hover: Color::srgb(0.88, 0.88, 0.92),
            popup_btn: Color::srgb(0.93, 0.93, 0.96),
            popup_btn_hover: Color::srgb(0.88, 0.88, 0.92),
            badge_ready: Color::srgb(0.15, 0.62, 0.25),
            badge_lyrics: Color::srgb(0.15, 0.48, 0.78),
            badge_not_analyzed: Color::srgb(0.62, 0.62, 0.66),
            badge_queued: Color::srgb(0.72, 0.58, 0.12),
            badge_analyzing: Color::srgb(0.85, 0.65, 0.08),
            badge_failed: Color::srgb(0.82, 0.22, 0.22),
            sung_color: Color::srgb(0.18, 0.42, 0.88),
            unsung_color: Color::srgba(0.15, 0.15, 0.2, 0.95),
            unsung_estimated: Color::srgba(0.6, 0.45, 0.1, 0.75),
            next_line_color: Color::srgba(0.15, 0.15, 0.2, 0.4),
            lyric_backdrop: Color::srgba(1.0, 1.0, 1.0, 0.6),
            lyric_backdrop_next: Color::srgba(1.0, 1.0, 1.0, 0.4),
            countdown_color: Color::srgb(0.18, 0.42, 0.88),
            countdown_bg: Color::srgba(1.0, 1.0, 1.0, 0.65),
            hud_primary: Color::WHITE,
            hud_secondary: Color::srgba(1.0, 1.0, 1.0, 0.6),
            hud_dim: Color::srgba(1.0, 1.0, 1.0, 0.5),
            pitch_ref_line: Srgba::new(0.2, 0.35, 0.75, 0.5),
            pitch_user_base: Srgba::new(0.1, 0.1, 0.25, 1.0),
            overlay_dim: Color::srgba(0.0, 0.0, 0.0, 0.6),
            spinner_overlay: Color::srgba(0.0, 0.0, 0.0, 0.55),
            skip_btn_bg: Color::srgba(0.0, 0.0, 0.0, 0.5),
            skip_btn_hover: Color::srgba(0.2, 0.2, 0.3, 0.7),
            skip_btn_border: Color::srgba(1.0, 1.0, 1.0, 0.4),
            skip_btn_text: Color::srgba(1.0, 1.0, 1.0, 0.8),
            star_gold: Color::srgb(1.0, 0.84, 0.0),
            cache_videos: Color::srgb(0.28, 0.72, 0.42),
            cache_models: Color::srgb(0.88, 0.62, 0.18),
            badge_video: Color::srgba(0.3, 0.42, 0.92, 0.8),
        }
    }

    pub fn from_config(config: &crate::config::AppConfig) -> Self {
        if config.is_dark_mode() {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub fn toggle(&mut self) {
        *self = match self.mode {
            ThemeMode::Dark => Self::light(),
            ThemeMode::Light => Self::dark(),
        };
    }

    pub fn mode_label(&self) -> &'static str {
        match self.mode {
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
        }
    }
}

pub fn spawn_label(
    parent: &mut ChildSpawnerCommands,
    text: impl Into<String>,
    size: f32,
    color: Color,
) {
    parent.spawn((
        Text::new(text),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    ));
}

pub enum ButtonVariant {
    Primary,
    Secondary,
    Danger,
    Sidebar,
}

impl ButtonVariant {
    fn bg(&self, theme: &UiTheme) -> Color {
        match self {
            Self::Primary => theme.accent,
            Self::Secondary => theme.popup_btn,
            Self::Danger => theme.badge_failed,
            Self::Sidebar => theme.sidebar_btn,
        }
    }

    fn text_color(&self, theme: &UiTheme) -> Color {
        match self {
            Self::Primary | Self::Danger => Color::WHITE,
            Self::Secondary => theme.text_primary,
            Self::Sidebar => theme.text_primary,
        }
    }

    fn font_size(&self) -> f32 {
        match self {
            Self::Sidebar => 13.0,
            _ => 14.0,
        }
    }

    fn border_radius(&self) -> f32 {
        5.0
    }
}

pub fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    variant: ButtonVariant,
    label: impl Into<String>,
    theme: &UiTheme,
    marker: impl Bundle,
) -> Entity {
    let text_color = variant.text_color(theme);
    spawn_button_with_color(parent, variant, label, theme, text_color, marker)
}

pub fn spawn_button_with_color(
    parent: &mut ChildSpawnerCommands,
    variant: ButtonVariant,
    label: impl Into<String>,
    theme: &UiTheme,
    text_color: Color,
    marker: impl Bundle,
) -> Entity {
    let bg = variant.bg(theme);
    let font_size = variant.font_size();
    let radius = variant.border_radius();

    parent
        .spawn((
            marker,
            Button,
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(
                    Val::Px(14.0),
                    Val::Px(14.0),
                    Val::Px(10.0),
                    Val::Px(10.0),
                ),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(radius)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::NONE),
            BackgroundColor(bg),
        ))
        .with_children(|btn| {
            spawn_label(btn, label, font_size, text_color);
        })
        .id()
}
