use crate::utilities::sound::{
    create_beep_sound, create_click_sound, create_cowbell_sound, create_hihat_sound,
    create_kick_sound, create_square_sound, create_triangle_sound, create_wood_block_sound,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundType {
    Beep,
    Kick,
    Click,
    Cowbell,
    Hihat,
    Square,
    Triangle,
    Woodblock,
}

impl Default for SoundType {
    fn default() -> Self {
        SoundType::Kick
    }
}

impl SoundType {
    pub const ALL: [SoundType; 8] = [
        SoundType::Beep,
        SoundType::Kick,
        SoundType::Click,
        SoundType::Cowbell,
        SoundType::Hihat,
        SoundType::Square,
        SoundType::Triangle,
        SoundType::Woodblock,
    ];

    pub fn next(&self) -> Self {
        let current_idx = Self::ALL.iter().position(|&s| s == *self).unwrap();
        Self::ALL[(current_idx + 1) % Self::ALL.len()]
    }

    pub fn prev(&self) -> Self {
        let current_idx = Self::ALL.iter().position(|&s| s == *self).unwrap();
        Self::ALL[(current_idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn name(&self) -> &'static str {
        match self {
            SoundType::Beep => "Beep",
            SoundType::Kick => "Kick",
            SoundType::Click => "Click",
            SoundType::Cowbell => "Cowbell",
            SoundType::Hihat => "Hi-hat",
            SoundType::Square => "Square",
            SoundType::Triangle => "Triangle",
            SoundType::Woodblock => "Woodblock",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SoundType::Beep => "🔔",
            SoundType::Kick => "🥁",
            SoundType::Click => "🖱️",
            SoundType::Cowbell => "🔔",
            SoundType::Hihat => "🎺",
            SoundType::Square => "⬜",
            SoundType::Triangle => "🔺",
            SoundType::Woodblock => "🪵",
        }
    }

    pub fn create_sound(&self) -> Vec<f32> {
        match self {
            SoundType::Beep => create_beep_sound(),
            SoundType::Kick => create_kick_sound(),
            SoundType::Click => create_click_sound(),
            SoundType::Cowbell => create_cowbell_sound(),
            SoundType::Hihat => create_hihat_sound(),
            SoundType::Square => create_square_sound(),
            SoundType::Triangle => create_triangle_sound(),
            SoundType::Woodblock => create_wood_block_sound(),
        }
    }
}
