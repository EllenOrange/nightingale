use bevy::prelude::*;

#[derive(Component)]
pub struct SongCard {
    pub song_index: usize,
}

#[derive(Component)]
pub struct SongListRoot;

#[derive(Component)]
pub struct SearchText;

#[derive(Component)]
pub struct StatusBadge {
    pub song_index: usize,
}

#[derive(Component)]
pub struct BadgeText {
    pub song_index: usize,
}

#[derive(Component)]
pub struct StatsText;

#[derive(Component)]
pub struct AlbumArtSlot {
    pub song_index: usize,
}

#[derive(Component)]
pub struct SpinnerOverlay {
    pub song_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction {
    ChangeFolder,
    Exit,
}

#[derive(Component)]
pub struct SidebarButton {
    pub action: SidebarAction,
}
