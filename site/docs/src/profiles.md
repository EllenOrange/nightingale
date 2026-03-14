# Profiles

Nightingale supports multiple player profiles for tracking scores across different singers.

## Creating Profiles

Create a new profile from the main menu. Each profile stores:

- Player name
- Per-song pitch scores and star ratings
- Score history

## Switching Profiles

Switch between profiles from the sidebar. The active profile is shown in the UI and all new scores are saved to it.

## Score Tracking

Scores are stored in `~/.nightingale/profiles.json`. Each profile maintains separate scoreboards for every song, so multiple singers can compete on the same library.
