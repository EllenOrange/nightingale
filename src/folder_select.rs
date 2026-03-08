use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use crate::config::AppConfig;
use crate::states::AppState;
use crate::ui;

pub struct FolderSelectPlugin;

impl Plugin for FolderSelectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::FolderSelect), show_folder_select)
            .add_systems(
                Update,
                (handle_folder_button, poll_folder_result)
                    .run_if(in_state(AppState::FolderSelect)),
            )
            .add_systems(OnExit(AppState::FolderSelect), cleanup_folder_select);
    }
}

#[derive(Component)]
struct FolderSelectRoot;

#[derive(Component)]
struct FolderSelectButton;

#[derive(Resource)]
struct PendingFolderPick {
    result: Arc<Mutex<Option<Option<PathBuf>>>>,
}

fn show_folder_select(mut commands: Commands) {
    commands
        .spawn((
            FolderSelectRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            BackgroundColor(ui::BG_COLOR),
        ))
        .with_children(|root| {
            ui::spawn_label(root, "KARASAD", 72.0, ui::ACCENT);

            ui::spawn_label(root, "Your own Karaoke", 24.0, ui::TEXT_SECONDARY);

            root.spawn((
                Text::new("Select a folder with your music to get started"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(ui::TEXT_DIM),
                Node {
                    margin: UiRect::top(Val::Px(16.0)),
                    ..default()
                },
            ));

            root.spawn((
                FolderSelectButton,
                Button,
                Node {
                    padding: UiRect::new(
                        Val::Px(32.0),
                        Val::Px(32.0),
                        Val::Px(16.0),
                        Val::Px(16.0),
                    ),
                    margin: UiRect::top(Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.3, 0.5, 0.9)),
            ))
            .with_children(|btn| {
                ui::spawn_label(btn, "Choose Music Folder", 20.0, Color::WHITE);
            });
        });
}

fn handle_folder_button(
    mut commands: Commands,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<FolderSelectButton>)>,
    pending: Option<Res<PendingFolderPick>>,
) {
    if pending.is_some() {
        return;
    }

    for interaction in &interaction_query {
        if *interaction == Interaction::Pressed {
            let result: Arc<Mutex<Option<Option<PathBuf>>>> = Arc::new(Mutex::new(None));
            let result_clone = Arc::clone(&result);

            std::thread::spawn(move || {
                let folder = rfd::FileDialog::new()
                    .set_title("Select your music folder")
                    .pick_folder();
                *result_clone.lock().unwrap() = Some(folder);
            });

            commands.insert_resource(PendingFolderPick { result });
        }
    }
}

fn poll_folder_result(
    mut commands: Commands,
    pending: Option<Res<PendingFolderPick>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut config: ResMut<AppConfig>,
) {
    let Some(pending) = pending else { return };

    let lock = pending.result.lock().unwrap();
    if let Some(ref maybe_folder) = *lock {
        if let Some(folder) = maybe_folder {
            info!("Selected folder: {}", folder.display());
            commands.insert_resource(crate::scanner::ScanRequest {
                folder: folder.clone(),
            });
            config.last_folder = Some(folder.clone());
            config.save();
            next_state.set(AppState::Scanning);
        }
        drop(lock);
        commands.remove_resource::<PendingFolderPick>();
    }
}

fn cleanup_folder_select(mut commands: Commands, query: Query<Entity, With<FolderSelectRoot>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}
