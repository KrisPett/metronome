use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use crate::utilities::sound_type::SoundType;

pub struct AtomicState {
    pub bpm: AtomicU32,
    pub is_running: AtomicBool,
    pub random_mode: AtomicBool,
    pub random_count: AtomicU32,
    pub remaining_ticks: AtomicU32,
    pub sound_type: AtomicU32,
    pub ui_dirty: AtomicBool,
    pub last_tick_time: AtomicU64,
    pub tick_count: AtomicU32,
    pub volume: AtomicU32,
}

impl AtomicState {
    pub fn new() -> Self {
        Self {
            bpm: AtomicU32::new(120),
            is_running: AtomicBool::new(false),
            random_mode: AtomicBool::new(false),
            random_count: AtomicU32::new(100),
            remaining_ticks: AtomicU32::new(0),
            sound_type: AtomicU32::new(1),
            ui_dirty: AtomicBool::new(true),
            last_tick_time: AtomicU64::new(0),
            tick_count: AtomicU32::new(0),
            volume: AtomicU32::new(80),
        }
    }

    pub fn get_sound_type(&self) -> SoundType {
        let index = self.sound_type.load(Ordering::Relaxed) as usize;
        SoundType::ALL[index.min(SoundType::ALL.len() - 1)]
    }

    pub fn set_sound_type(&self, sound_type: SoundType) {
        if let Some(index) = SoundType::ALL.iter().position(|&s| s == sound_type) {
            self.sound_type.store(index as u32, Ordering::Relaxed);
        }
    }

    pub fn update_tick(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.last_tick_time.store(now, Ordering::Relaxed);
        self.tick_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_last_tick_elapsed(&self) -> Duration {
        let last_tick_nanos = self.last_tick_time.load(Ordering::Relaxed);
        if last_tick_nanos == 0 {
            return Duration::from_secs(999);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        Duration::from_nanos(now.saturating_sub(last_tick_nanos))
    }
}
