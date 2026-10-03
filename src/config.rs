// Copyright 2023 System76 <info@system76.com>
// Copyright 2026 Daniel Probst <daenuprobst@gmail.com>
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::{
    Theme,
    cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry},
    iced::Color,
};
use serde::{Deserialize, Serialize};

pub const APP_ID: &str = "dev.daenu.CosmicExtAppletLowkeyWorkspaces";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct WorkspacesConfig {
    pub active_color: ActiveColor,
    pub inactive_opacity: f32,
    pub show_dots: bool,
    pub show_circles: bool,
    pub circle_size: Option<u16>,
    pub invert_numbers: bool,
    pub spacing: Option<u16>,
    /// Per-workspace label replacing the number, empty keeps the number.
    pub icons: Vec<String>,
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        Self {
            active_color: ActiveColor::Accent,
            inactive_opacity: 1.0,
            show_dots: false,
            show_circles: false,
            circle_size: None,
            invert_numbers: false,
            spacing: None,
            icons: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveColor {
    #[default]
    Accent,
    Text,
    Success,
    Warning,
    Destructive,
    Blue,
    Indigo,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    WarmGrey,
}

impl ActiveColor {
    pub const ALL: [Self; 14] = [
        Self::Accent,
        Self::Text,
        Self::Success,
        Self::Warning,
        Self::Destructive,
        Self::Blue,
        Self::Indigo,
        Self::Purple,
        Self::Pink,
        Self::Red,
        Self::Orange,
        Self::Yellow,
        Self::Green,
        Self::WarmGrey,
    ];

    pub fn color(self, theme: &Theme) -> Color {
        let cosmic = theme.cosmic();
        let palette = &cosmic.palette;
        match self {
            Self::Accent => cosmic.accent_text_color(),
            Self::Text => theme.current_container().component.on,
            Self::Success => cosmic.success_text_color(),
            Self::Warning => cosmic.warning_text_color(),
            Self::Destructive => cosmic.destructive_text_color(),
            Self::Blue => palette.accent_blue,
            Self::Indigo => palette.accent_indigo,
            Self::Purple => palette.accent_purple,
            Self::Pink => palette.accent_pink,
            Self::Red => palette.accent_red,
            Self::Orange => palette.accent_orange,
            Self::Yellow => palette.accent_yellow,
            Self::Green => palette.accent_green,
            Self::WarmGrey => palette.accent_warm_grey,
        }
        .into()
    }
}
