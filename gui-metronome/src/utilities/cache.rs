use std::collections::HashMap;

use crate::utilities::sound_type::SoundType;

pub struct SoundCache {
    sounds: HashMap<SoundType, Vec<f32>>,
}

impl SoundCache {
    pub fn new() -> Self {
        let mut sounds = HashMap::new();
        for &sound_type in &SoundType::ALL {
            sounds.insert(sound_type, sound_type.create_sound());
        }
        Self { sounds }
    }

    pub fn get_sound(&self, sound_type: SoundType) -> &Vec<f32> {
        &self.sounds[&sound_type]
    }
}

#[derive(Default)]
pub struct UICache {
    pub last_bpm: u32,
    pub last_sound: SoundType,
    pub last_status: bool,
    pub last_random_mode: bool,
    pub last_remaining_ticks: u32,
    pub last_random_count: u32,
    pub last_tick_count: u32,
    pub last_volume: u32,
    pub first_render: bool,
    pub animation_buffer: String,
    pub last_animation_frame: usize,
}

impl UICache {
    pub fn new() -> Self {
        Self {
            first_render: true,
            animation_buffer: String::with_capacity(100),
            ..Default::default()
        }
    }
}
