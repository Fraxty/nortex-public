//! Density profiles with logical (DPI-independent) spacing values.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub panel_padding: f32,
    pub item_horizontal: f32,
    pub item_vertical: f32,
    pub button_horizontal: f32,
    pub button_vertical: f32,
    pub editor_line_spacing: f32,
}

impl Density {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Comfortable, Self::Spacious];

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Comfortable => "Comfortable",
            Self::Spacious => "Spacious",
        }
    }
    /// Kurzbeschreibung für die Auswahlliste.
    pub fn description(self) -> &'static str {
        match self {
            Self::Compact => "viel Inhalt, wenig Abstand",
            Self::Comfortable => "ausgewogene Abstände",
            Self::Spacious => "großzügige Abstände",
        }
    }
    pub fn spacing(self) -> Spacing {
        match self {
            Self::Compact => Spacing {
                panel_padding: 8.0,
                item_horizontal: 6.0,
                item_vertical: 4.0,
                button_horizontal: 10.0,
                button_vertical: 5.0,
                editor_line_spacing: 1.2,
            },
            Self::Comfortable => Spacing {
                panel_padding: 12.0,
                item_horizontal: 8.0,
                item_vertical: 6.0,
                button_horizontal: 12.0,
                button_vertical: 7.0,
                editor_line_spacing: 1.4,
            },
            Self::Spacious => Spacing {
                panel_padding: 16.0,
                item_horizontal: 12.0,
                item_vertical: 9.0,
                button_horizontal: 15.0,
                button_vertical: 9.0,
                editor_line_spacing: 1.6,
            },
        }
    }
}
