use std::sync::{mpsc, Mutex};

use bevy::prelude::*;

use crate::states::AppState;
use crate::ui::UiTheme;
use crate::vendor::{self, BootstrapProgress};

pub struct SetupPlugin;

impl Plugin for SetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Setup), spawn_setup_ui)
            .add_systems(
                Update,
                poll_progress.run_if(in_state(AppState::Setup)),
            )
            .add_systems(OnExit(AppState::Setup), cleanup_setup);
    }
}

#[derive(Component)]
struct SetupRoot;

#[derive(Component)]
struct StepText;

#[derive(Component)]
struct DetailText;

#[derive(Component)]
struct ProgressBar;

#[derive(Resource)]
struct BootstrapReceiver(Mutex<mpsc::Receiver<BootstrapProgress>>);

fn spawn_setup_ui(mut commands: Commands, theme: Res<UiTheme>, asset_server: Res<AssetServer>) {
    let (tx, rx) = mpsc::channel();
    commands.insert_resource(BootstrapReceiver(Mutex::new(rx)));

    std::thread::spawn(move || {
        vendor::run_bootstrap(tx);
    });

    commands
        .spawn((
            SetupRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(theme.bg),
        ))
        .with_children(|root| {
            root.spawn((
                ImageNode::new(asset_server.load("images/logo.png")),
                Node {
                    width: Val::Px(300.0),
                    ..default()
                },
            ));

            root.spawn((
                Text::new("Setting up for first launch..."),
                TextFont {
                    font_size: 18.0,
                    ..default()
                },
                TextColor(theme.text_secondary),
            ));

            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                width: Val::Px(400.0),
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    StepText,
                    Text::new("Preparing..."),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(theme.accent),
                ));

                col.spawn((
                    DetailText,
                    Text::new(""),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(theme.text_dim),
                ));

                col.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|bar_bg| {
                    bar_bg.spawn((
                        ProgressBar,
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(theme.accent),
                    ));
                });

                col.spawn((
                    Text::new("This downloads ~1-3 GB of ML models and tools.\nIt only happens once."),
                    TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    TextColor(theme.text_dim),
                    Node {
                        margin: UiRect::top(Val::Px(8.0)),
                        ..default()
                    },
                ));
            });
        });
}

const STEP_WEIGHTS: &[(&str, f32)] = &[
    ("ffmpeg", 10.0),
    ("uv", 10.0),
    ("Python", 10.0),
    ("Venv", 5.0),
    ("Packages", 60.0),
    ("Scripts", 5.0),
];

fn step_progress(step: &str) -> f32 {
    let mut cumulative = 0.0;
    for &(name, weight) in STEP_WEIGHTS {
        if name == step {
            return cumulative + weight * 0.5;
        }
        cumulative += weight;
    }
    cumulative
}

fn poll_progress(
    receiver: Option<Res<BootstrapReceiver>>,
    mut step_text: Query<&mut Text, (With<StepText>, Without<DetailText>, Without<ProgressBar>)>,
    mut detail_text: Query<&mut Text, (With<DetailText>, Without<StepText>, Without<ProgressBar>)>,
    mut bar: Query<&mut Node, With<ProgressBar>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Some(receiver) = receiver else { return };
    let Ok(rx) = receiver.0.lock() else { return };

    let mut latest = None;
    while let Ok(msg) = rx.try_recv() {
        latest = Some(msg);
    }
    drop(rx);

    let Some(progress) = latest else { return };

    if let Ok(mut text) = step_text.single_mut() {
        if progress.error.is_some() {
            **text = "Setup failed".into();
        } else {
            **text = progress.step.to_string();
        }
    }

    if let Ok(mut text) = detail_text.single_mut() {
        if let Some(ref err) = progress.error {
            **text = err.clone();
        } else {
            **text = progress.detail.clone();
        }
    }

    if let Ok(mut node) = bar.single_mut() {
        let pct = if progress.done && progress.error.is_none() {
            100.0
        } else {
            step_progress(progress.step)
        };
        node.width = Val::Percent(pct);
    }

    if progress.done && progress.error.is_none() {
        next_state.set(AppState::Menu);
    }
}

fn cleanup_setup(mut commands: Commands, roots: Query<Entity, With<SetupRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<BootstrapReceiver>();
}
