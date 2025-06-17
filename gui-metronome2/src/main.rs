#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use rand::Rng;
use rodio::{OutputStream, OutputStreamHandle, Sink, buffer::SamplesBuffer};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    mpsc::{self, Receiver, Sender},
};
use std::thread;
use std::time::{Duration, Instant};

mod utilities;
use crate::utilities::sound::{
    create_beep_sound, create_click_sound, create_cowbell_sound, create_hihat_sound,
    create_kick_sound, create_square_sound, create_triangle_sound, create_wood_block_sound,
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum MetronomeMode {
    Standard,
    Random,
    Practice,
    Polyrhythm,
    Ritardando,
    Subdivision,
    Countdown,
}

// Commands sent to the metronome thread
#[derive(Debug, Clone)]
enum MetronomeCommand {
    Start,
    Stop,
    ChangeBpm(u32),
    ChangeVolume(u32),
    ChangeSoundType(u32),
    ChangeMode(MetronomeMode),
    UpdateRandomSettings { count: u32 },
    UpdatePracticeSettings { sections: Vec<(u32, u32)> },
    UpdatePolyrhythmSettings { primary: u32, secondary: u32, accent_primary: bool, accent_secondary: bool },
    UpdateRitardandoSettings { start_bpm: u32, target_bpm: u32, duration: u32 },
    UpdateSubdivisionSettings { subdivisions: u32, pattern: Vec<bool> },
    UpdateCountdownSettings { duration_seconds: u32, enable_random_bpm: bool },
    Reset,
}

// Events sent back from the metronome thread
#[derive(Debug, Clone)]
enum MetronomeEvent {
    Beat { tick_count: u32, is_accent: bool },
    ModeChanged { mode: MetronomeMode },
    BpmChanged { bpm: u32 },
    CountdownFinished,
    Error { message: String },
}

struct MetronomeApp {
    // Communication with metronome thread
    command_sender: Sender<MetronomeCommand>,
    event_receiver: Receiver<MetronomeEvent>,
    
    // Shared state (read-only from UI thread)
    shared_state: Arc<SharedMetronomeState>,
    
    // UI-only state
    animation_progress: f32,
    beat_progress: f32,
    last_beat_time: Instant,
    celebration_animation: f32,
    celebration_time: Instant,
    
    // Audio resources
    _stream: OutputStream,
    #[allow(dead_code)]
    stream_handle: OutputStreamHandle,
}

// Thread-safe shared state
struct SharedMetronomeState {
    // Core state
    bpm: AtomicU32,
    is_running: AtomicBool,
    volume: AtomicU32,
    sound_type: AtomicU32,
    tick_count: AtomicU32,
    
    // Current mode (atomic for simple reads)
    mode: AtomicUsize, // We'll cast MetronomeMode to/from usize
    
    // Mode-specific state (protected by RwLock for complex data)
    random_state: RwLock<RandomState>,
    practice_state: RwLock<PracticeState>,
    polyrhythm_state: RwLock<PolyrhythmState>,
    ritardando_state: RwLock<RitardandoState>,
    subdivision_state: RwLock<SubdivisionState>,
    countdown_state: RwLock<CountdownState>,
    
    // Beat timing
    last_beat: RwLock<Instant>,
}

#[derive(Clone, Debug)]
struct RandomState {
    count: u32,
    remaining_ticks: u32,
}

#[derive(Clone, Debug)]
struct PracticeState {
    sections: Vec<(u32, u32)>, // (BPM, beats)
    current_section: u32,
    section_remaining: u32,
}

#[derive(Clone, Debug)]
struct PolyrhythmState {
    primary: u32,
    secondary: u32,
    accent_primary: bool,
    accent_secondary: bool,
}

#[derive(Clone, Debug)]
struct RitardandoState {
    start_bpm: u32,
    target_bpm: u32,
    duration: u32,
    remaining: u32,
}

#[derive(Clone, Debug)]
struct SubdivisionState {
    subdivisions: u32,
    accent_pattern: Vec<bool>,
}

#[derive(Clone, Debug)]
struct CountdownState {
    duration_seconds: u32,
    remaining_seconds: f32,
    enable_random_bpm: bool,
    original_bpm: u32,
    next_bpm_change: f32,
}

// Helper function to create celebration sound
fn create_celebration_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration = 2.0; // 2 seconds
    let mut samples = Vec::new();
    
    // Create a celebratory chord progression
    let frequencies = [
        [523.25, 659.25, 783.99], // C major chord
        [587.33, 739.99, 880.0],  // D major chord
        [659.25, 830.61, 987.77], // E major chord
        [698.46, 880.0, 1046.5],  // F major chord
    ];
    
    for chord_idx in 0..frequencies.len() {
        let chord_duration = duration / frequencies.len() as f32;
        let chord_samples = (sample_rate as f32 * chord_duration) as usize;
        
        for i in 0..chord_samples {
            let t = i as f32 / sample_rate as f32;
            let mut sample = 0.0;
            
            // Add each note in the chord
            for &freq in &frequencies[chord_idx] {
                sample += (t * freq * 2.0 * PI).sin() * 0.2;
            }
            
            // Add some envelope
            let envelope = if t < 0.1 {
                t / 0.1
            } else if t > chord_duration - 0.1 {
                (chord_duration - t) / 0.1
            } else {
                1.0
            };
            
            samples.push(sample * envelope);
        }
    }
    
    samples
}

impl Default for MetronomeApp {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        
        // Create communication channels
        let (command_sender, command_receiver) = mpsc::channel();
        let (event_sender, event_receiver) = mpsc::channel();
        
        // Initialize shared state
        let shared_state = Arc::new(SharedMetronomeState {
            bpm: AtomicU32::new(120),
            is_running: AtomicBool::new(false),
            volume: AtomicU32::new(80),
            sound_type: AtomicU32::new(0),
            tick_count: AtomicU32::new(0),
            mode: AtomicUsize::new(MetronomeMode::Standard as usize),
            random_state: RwLock::new(RandomState {
                count: 100,
                remaining_ticks: 100,
            }),
            practice_state: RwLock::new(PracticeState {
                sections: vec![(60, 32), (120, 32), (180, 32)],
                current_section: 0,
                section_remaining: 0,
            }),
            polyrhythm_state: RwLock::new(PolyrhythmState {
                primary: 4,
                secondary: 3,
                accent_primary: true,
                accent_secondary: true,
            }),
            ritardando_state: RwLock::new(RitardandoState {
                start_bpm: 120,
                target_bpm: 180,
                duration: 64,
                remaining: 0,
            }),
            subdivision_state: RwLock::new(SubdivisionState {
                subdivisions: 1,
                accent_pattern: vec![true, false, false, false],
            }),
            countdown_state: RwLock::new(CountdownState {
                duration_seconds: 60,
                remaining_seconds: 60.0,
                enable_random_bpm: false,
                original_bpm: 120,
                next_bpm_change: 5.0,
            }),
            last_beat: RwLock::new(Instant::now()),
        });

        // Create sound cache including celebration sound
        let mut sound_cache = HashMap::new();
        for i in 0..9 { // Increased to include celebration sound
            let sound_data = match i {
                0 => create_beep_sound(),
                1 => create_kick_sound(),
                2 => create_click_sound(),
                3 => create_cowbell_sound(),
                4 => create_hihat_sound(),
                5 => create_wood_block_sound(),
                6 => create_triangle_sound(),
                7 => create_square_sound(),
                8 => create_celebration_sound(), // New celebration sound
                _ => create_beep_sound(),
            };
            sound_cache.insert(i, sound_data);
        }

        // Start metronome thread
        let shared_state_clone = Arc::clone(&shared_state);
        let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle).unwrap()));
        
        thread::spawn(move || {
            metronome_thread(shared_state_clone, sink, sound_cache, command_receiver, event_sender);
        });

        Self {
            command_sender,
            event_receiver,
            shared_state,
            animation_progress: 0.0,
            beat_progress: 0.0,
            last_beat_time: Instant::now(),
            celebration_animation: 0.0,
            celebration_time: Instant::now(),
            _stream,
            stream_handle,
        }
    }
}

impl SharedMetronomeState {
    fn get_mode(&self) -> MetronomeMode {
        let mode_val = self.mode.load(Ordering::Relaxed);
        match mode_val {
            0 => MetronomeMode::Standard,
            1 => MetronomeMode::Random,
            2 => MetronomeMode::Practice,
            3 => MetronomeMode::Polyrhythm,
            4 => MetronomeMode::Ritardando,
            5 => MetronomeMode::Subdivision,
            6 => MetronomeMode::Countdown,
            _ => MetronomeMode::Standard,
        }
    }
    
    fn set_mode(&self, mode: MetronomeMode) {
        self.mode.store(mode as usize, Ordering::Relaxed);
    }
}

struct Theme {
    primary: egui::Color32,
    secondary: egui::Color32,
    accent: egui::Color32,
    background: egui::Color32,
    surface: egui::Color32,
    on_surface: egui::Color32,
    error: egui::Color32,
    success: egui::Color32,
    warning: egui::Color32,
    polyrhythm: egui::Color32,
    practice: egui::Color32,
    countdown: egui::Color32,
}

impl Theme {
    fn dark() -> Self {
        Self {
            primary: egui::Color32::from_rgb(138, 43, 226),
            secondary: egui::Color32::from_rgb(75, 0, 130),
            accent: egui::Color32::from_rgb(255, 140, 0),
            background: egui::Color32::from_rgb(18, 18, 18),
            surface: egui::Color32::from_rgb(32, 32, 32),
            on_surface: egui::Color32::from_rgb(220, 220, 220),
            error: egui::Color32::from_rgb(244, 67, 54),
            success: egui::Color32::from_rgb(76, 175, 80),
            warning: egui::Color32::from_rgb(255, 193, 7),
            polyrhythm: egui::Color32::from_rgb(255, 64, 129),
            practice: egui::Color32::from_rgb(0, 188, 212),
            countdown: egui::Color32::from_rgb(255, 87, 34),
        }
    }
}

fn metronome_thread(
    state: Arc<SharedMetronomeState>,
    sink: Arc<Mutex<Sink>>,
    sound_cache: HashMap<u32, Vec<f32>>,
    command_receiver: Receiver<MetronomeCommand>,
    event_sender: Sender<MetronomeEvent>,
) {
    let mut last_tick = Instant::now();
    let mut subdivision_tick = 0u32;
    let mut countdown_start_time = Instant::now();
    
    // Local state for the metronome thread
    let mut local_random_state = state.random_state.read().unwrap().clone();
    let mut local_practice_state = state.practice_state.read().unwrap().clone();
    let mut local_polyrhythm_state = state.polyrhythm_state.read().unwrap().clone();
    let mut local_ritardando_state = state.ritardando_state.read().unwrap().clone();
    let mut local_subdivision_state = state.subdivision_state.read().unwrap().clone();
    let mut local_countdown_state = state.countdown_state.read().unwrap().clone();

    loop {
        // Process commands (non-blocking)
        while let Ok(command) = command_receiver.try_recv() {
            match command {
                MetronomeCommand::Start => {
                    state.is_running.store(true, Ordering::Relaxed);
                    state.tick_count.store(0, Ordering::Relaxed);
                    last_tick = Instant::now();
                    countdown_start_time = Instant::now();
                    subdivision_tick = 0;
                    
                    // Reset mode-specific state
                    let current_mode = state.get_mode();
                    match current_mode {
                        MetronomeMode::Random => {
                            local_random_state.remaining_ticks = local_random_state.count;
                        },
                        MetronomeMode::Practice => {
                            local_practice_state.current_section = 0;
                            local_practice_state.section_remaining = 0;
                        },
                        MetronomeMode::Ritardando => {
                            local_ritardando_state.remaining = local_ritardando_state.duration;
                            state.bpm.store(local_ritardando_state.start_bpm, Ordering::Relaxed);
                        },
                        MetronomeMode::Countdown => {
                            local_countdown_state.remaining_seconds = local_countdown_state.duration_seconds as f32;
                            local_countdown_state.original_bpm = state.bpm.load(Ordering::Relaxed);
                            local_countdown_state.next_bpm_change = 5.0; // Change BPM every 5 seconds
                        },
                        _ => {},
                    }
                },
                MetronomeCommand::Stop => {
                    state.is_running.store(false, Ordering::Relaxed);
                },
                MetronomeCommand::ChangeBpm(bpm) => {
                    state.bpm.store(bpm, Ordering::Relaxed);
                },
                MetronomeCommand::ChangeVolume(volume) => {
                    state.volume.store(volume, Ordering::Relaxed);
                },
                MetronomeCommand::ChangeSoundType(sound_type) => {
                    state.sound_type.store(sound_type, Ordering::Relaxed);
                },
                MetronomeCommand::ChangeMode(mode) => {
                    state.set_mode(mode);
                    let _ = event_sender.send(MetronomeEvent::ModeChanged { mode });
                },
                MetronomeCommand::UpdateRandomSettings { count } => {
                    local_random_state.count = count;
                    local_random_state.remaining_ticks = count;
                    *state.random_state.write().unwrap() = local_random_state.clone();
                },
                MetronomeCommand::UpdatePracticeSettings { sections } => {
                    local_practice_state.sections = sections;
                    *state.practice_state.write().unwrap() = local_practice_state.clone();
                },
                MetronomeCommand::UpdatePolyrhythmSettings { primary, secondary, accent_primary, accent_secondary } => {
                    local_polyrhythm_state = PolyrhythmState {
                        primary,
                        secondary,
                        accent_primary,
                        accent_secondary,
                    };
                    *state.polyrhythm_state.write().unwrap() = local_polyrhythm_state.clone();
                },
                MetronomeCommand::UpdateRitardandoSettings { start_bpm, target_bpm, duration } => {
                    local_ritardando_state.start_bpm = start_bpm;
                    local_ritardando_state.target_bpm = target_bpm;
                    local_ritardando_state.duration = duration.max(1);
                    *state.ritardando_state.write().unwrap() = local_ritardando_state.clone();
                },
                MetronomeCommand::UpdateSubdivisionSettings { subdivisions, pattern } => {
                    local_subdivision_state.subdivisions = subdivisions;
                    local_subdivision_state.accent_pattern = pattern;
                    *state.subdivision_state.write().unwrap() = local_subdivision_state.clone();
                },
                MetronomeCommand::UpdateCountdownSettings { duration_seconds, enable_random_bpm } => {
                    local_countdown_state.duration_seconds = duration_seconds;
                    local_countdown_state.enable_random_bpm = enable_random_bpm;
                    *state.countdown_state.write().unwrap() = local_countdown_state.clone();
                },
                MetronomeCommand::Reset => {
                    state.tick_count.store(0, Ordering::Relaxed);
                    subdivision_tick = 0;
                },
            }
        }

        if state.is_running.load(Ordering::Relaxed) {
            let current_mode = state.get_mode();
            let mut effective_bpm = state.bpm.load(Ordering::Relaxed);
            let mut should_tick = false;
            let mut is_accent = false;
            let mut use_alternate_sound = false;

            // Handle countdown mode timing
            if current_mode == MetronomeMode::Countdown {
                let elapsed = countdown_start_time.elapsed().as_secs_f32();
                local_countdown_state.remaining_seconds = (local_countdown_state.duration_seconds as f32 - elapsed).max(0.0);
                
                // Check if countdown finished
                if local_countdown_state.remaining_seconds <= 0.0 {
                    state.is_running.store(false, Ordering::Relaxed);
                    
                    // Play celebration sound
                    let volume = state.volume.load(Ordering::Relaxed) as f32 / 100.0;
                    if let Some(celebration_sound) = sound_cache.get(&8) {
                        let volume_adjusted_sound: Vec<f32> = celebration_sound
                            .iter()
                            .map(|&sample| sample * volume * 1.5) // Louder for celebration
                            .collect();
                        
                        let source = SamplesBuffer::new(1, 44100, volume_adjusted_sound);
                        if let Ok(sink_guard) = sink.try_lock() {
                            sink_guard.append(source);
                        }
                    }
                    
                    let _ = event_sender.send(MetronomeEvent::CountdownFinished);
                    continue;
                }
                
                // Handle random BPM changes during countdown
                if local_countdown_state.enable_random_bpm {
                    local_countdown_state.next_bpm_change -= elapsed - (local_countdown_state.duration_seconds as f32 - local_countdown_state.remaining_seconds);
                    
                    if local_countdown_state.next_bpm_change <= 0.0 {
                        let mut rng = rand::thread_rng();
                        let new_bpm = rng.gen_range(80..=180);
                        state.bpm.store(new_bpm, Ordering::Relaxed);
                        local_countdown_state.next_bpm_change = rng.gen_range(3.0..=8.0); // Next change in 3-8 seconds
                        let _ = event_sender.send(MetronomeEvent::BpmChanged { bpm: new_bpm });
                    }
                }
                
                // Update shared countdown state
                if let Ok(mut shared_countdown) = state.countdown_state.try_write() {
                    *shared_countdown = local_countdown_state.clone();
                }
            }

            // Calculate beat interval based on mode
            let beat_interval = match current_mode {
                MetronomeMode::Subdivision => {
                    let multiplier = match local_subdivision_state.subdivisions {
                        1 => 1.0,  // Quarter notes
                        2 => 2.0,  // Eighth notes
                        3 => 3.0,  // Triplets
                        4 => 4.0,  // Sixteenth notes
                        _ => 1.0,
                    };
                    Duration::from_millis((60000.0 / (effective_bpm as f32 * multiplier)) as u64)
                },
                _ => Duration::from_millis(60000 / effective_bpm.max(1) as u64),
            };

            if last_tick.elapsed() >= beat_interval {
                should_tick = true;
                
                match current_mode {
                    MetronomeMode::Standard => {
                        // Standard mode - just tick
                    },
                    
                    MetronomeMode::Countdown => {
                        // Countdown mode - accent every 10 seconds
                        let seconds_elapsed = local_countdown_state.duration_seconds as f32 - local_countdown_state.remaining_seconds;
                        if seconds_elapsed % 10.0 < 0.5 {
                            is_accent = true;
                        }
                    },
                    
                    MetronomeMode::Random => {
                        if local_random_state.remaining_ticks == 0 {
                            local_random_state.remaining_ticks = local_random_state.count;
                        }
                        
                        local_random_state.remaining_ticks = local_random_state.remaining_ticks.saturating_sub(1);
                        
                        if local_random_state.remaining_ticks == 0 {
                            let mut rng = rand::thread_rng();
                            let new_bpm = rng.gen_range(60..=200);
                            state.bpm.store(new_bpm, Ordering::Relaxed);
                            let _ = event_sender.send(MetronomeEvent::BpmChanged { bpm: new_bpm });
                        }
                        
                        if let Ok(mut shared_random) = state.random_state.try_write() {
                            *shared_random = local_random_state.clone();
                        }
                    },
                    
                    MetronomeMode::Practice => {
                        if local_practice_state.section_remaining == 0 {
                            let current_section = local_practice_state.current_section as usize;
                            
                            if current_section < local_practice_state.sections.len() {
                                let (section_bpm, section_beats) = local_practice_state.sections[current_section];
                                state.bpm.store(section_bpm, Ordering::Relaxed);
                                local_practice_state.section_remaining = section_beats;
                                
                                let next_section = (current_section + 1) % local_practice_state.sections.len();
                                local_practice_state.current_section = next_section as u32;
                                
                                let _ = event_sender.send(MetronomeEvent::BpmChanged { bpm: section_bpm });
                            }
                        }
                        
                        local_practice_state.section_remaining = local_practice_state.section_remaining.saturating_sub(1);
                        
                        if let Ok(mut shared_practice) = state.practice_state.try_write() {
                            *shared_practice = local_practice_state.clone();
                        }
                    },
                    
                    MetronomeMode::Polyrhythm => {
                        let tick_count = state.tick_count.load(Ordering::Relaxed);
                        
                        let primary_hit = local_polyrhythm_state.primary > 0 && (tick_count % local_polyrhythm_state.primary) == 0;
                        let secondary_hit = local_polyrhythm_state.secondary > 0 && (tick_count % local_polyrhythm_state.secondary) == 0;
                        
                        if primary_hit && local_polyrhythm_state.accent_primary {
                            is_accent = true;
                        }
                        if secondary_hit && local_polyrhythm_state.accent_secondary {
                            use_alternate_sound = true;
                        }
                    },
                    
                    MetronomeMode::Ritardando => {
                        if local_ritardando_state.remaining == 0 {
                            local_ritardando_state.remaining = local_ritardando_state.duration;
                        }
                        
                        let start_bpm = local_ritardando_state.start_bpm as f32;
                        let target_bpm = local_ritardando_state.target_bpm as f32;
                        let duration = local_ritardando_state.duration as f32;
                        
                        if duration > 0.0 {
                            let progress = (duration - local_ritardando_state.remaining as f32) / duration;
                            let current_bpm = start_bpm - (start_bpm - target_bpm) * progress;
                            let current_bpm_u32 = (current_bpm as u32).max(1);
                            state.bpm.store(current_bpm_u32, Ordering::Relaxed);
                        } else {
                            state.bpm.store(local_ritardando_state.target_bpm, Ordering::Relaxed);
                        }
                        
                        local_ritardando_state.remaining = local_ritardando_state.remaining.saturating_sub(1);
                        
                        if let Ok(mut shared_ritardando) = state.ritardando_state.try_write() {
                            *shared_ritardando = local_ritardando_state.clone();
                        }
                    },
                    
                    MetronomeMode::Subdivision => {
                        if !local_subdivision_state.accent_pattern.is_empty() {
                            let pattern_index = subdivision_tick as usize % local_subdivision_state.accent_pattern.len();
                            is_accent = local_subdivision_state.accent_pattern[pattern_index];
                        }
                        
                        subdivision_tick = subdivision_tick.wrapping_add(1);
                    },
                }

                if should_tick {
                    let new_tick_count = state.tick_count.fetch_add(1, Ordering::Relaxed) + 1;

                    if let Ok(mut last_beat) = state.last_beat.try_write() {
                        *last_beat = Instant::now();
                    }

                    let _ = event_sender.send(MetronomeEvent::Beat {
                        tick_count: new_tick_count,
                        is_accent,
                    });

                    // Play sound
                    let volume = state.volume.load(Ordering::Relaxed) as f32 / 100.0;
                    let mut sound_type = state.sound_type.load(Ordering::Relaxed);
                    
                    if use_alternate_sound {
                        sound_type = (sound_type + 1) % 8;
                    }
                    
                    let final_volume = if is_accent { 
                        (volume * 1.5).min(1.0)
                    } else { 
                        volume 
                    };

                    if let Some(sound_data) = sound_cache.get(&sound_type) {
                        let volume_adjusted_sound: Vec<f32> =
                            sound_data.iter().map(|&sample| sample * final_volume).collect();

                        let source = SamplesBuffer::new(1, 44100, volume_adjusted_sound);

                        if let Ok(sink_guard) = sink.try_lock() {
                            sink_guard.append(source);
                        }
                    }
                }

                last_tick = Instant::now();
            }
        } else {
            last_tick = Instant::now();
            subdivision_tick = 0;
        }

        thread::sleep(Duration::from_millis(1));
    }
}

impl eframe::App for MetronomeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process events from metronome thread
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                MetronomeEvent::Beat { is_accent, .. } => {
                    self.last_beat_time = Instant::now();
                },
                MetronomeEvent::CountdownFinished => {
                    self.celebration_time = Instant::now();
                    self.celebration_animation = 1.0;
                },
                MetronomeEvent::ModeChanged { .. } => {},
                MetronomeEvent::BpmChanged { .. } => {},
                MetronomeEvent::Error { message } => {
                    eprintln!("Metronome error: {}", message);
                },
            }
        }

        let theme = Theme::dark();

        let mut style = (*ctx.style()).clone();
        style.visuals.dark_mode = true;
        style.visuals.override_text_color = Some(theme.on_surface);
        style.visuals.panel_fill = theme.background;
        style.visuals.window_fill = theme.surface;
        style.visuals.extreme_bg_color = theme.surface;
        style.visuals.faint_bg_color = theme.surface;
        style.visuals.widgets.inactive.bg_fill = theme.surface;
        style.visuals.widgets.hovered.bg_fill = theme.primary;
        style.visuals.widgets.active.bg_fill = theme.secondary;
        style.spacing.slider_width = 200.0;
        style.spacing.button_padding = egui::vec2(16.0, 12.0);
        style.spacing.item_spacing = egui::vec2(12.0, 8.0);
        style.spacing.indent = 25.0;
        ctx.set_style(style);

        let bpm = self.shared_state.bpm.load(Ordering::Relaxed);
        let is_running = self.shared_state.is_running.load(Ordering::Relaxed);
        let volume = self.shared_state.volume.load(Ordering::Relaxed);
        let tick_count = self.shared_state.tick_count.load(Ordering::Relaxed);
        let current_mode = self.shared_state.get_mode();

        // Handle celebration animation
        if self.celebration_animation > 0.0 {
            let elapsed = self.celebration_time.elapsed().as_secs_f32();
            self.celebration_animation = (3.0 - elapsed).max(0.0) / 3.0;
            ctx.request_repaint();
        }

        if is_running {
            if let Ok(last_beat) = self.shared_state.last_beat.try_read() {
                let time_since_beat = last_beat.elapsed().as_millis() as f32;
                let effective_bpm = match current_mode {
                    MetronomeMode::Subdivision => {
                        if let Ok(subdivision_state) = self.shared_state.subdivision_state.try_read() {
                            let multiplier = match subdivision_state.subdivisions {
                                1 => 1.0, 2 => 2.0, 3 => 3.0, 4 => 4.0, _ => 1.0,
                            };
                            bpm as f32 * multiplier
                        } else {
                            bpm as f32
                        }
                    },
                    _ => bpm as f32,
                };
                let beat_interval_ms = 60000.0 / effective_bpm.max(1.0);

                self.beat_progress = (time_since_beat / beat_interval_ms).min(1.0);

                if time_since_beat < 200.0 {
                    self.animation_progress = 1.0 - (time_since_beat / 200.0);
                } else {
                    self.animation_progress = 0.0;
                }
            }
            ctx.request_repaint();
        } else {
            self.animation_progress = 0.0;
            self.beat_progress = 0.0;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                
                // Show celebration effects if active
                if self.celebration_animation > 0.0 {
                    ui.heading(
                        egui::RichText::new("🎉 COUNTDOWN COMPLETE! 🎉")
                            .size(40.0)
                            .color(egui::Color32::from_rgb(255, 215, 0))
                            .strong(),
                    );
                    ui.add_space(10.0);
                }
                
                ui.heading(
                    egui::RichText::new("🎵 METRONOME STUDIO PRO")
                        .size(32.0)
                        .color(theme.primary)
                        .strong(),
                );
                ui.add_space(10.0);

                let separator_rect = ui
                    .allocate_space([ui.available_width() - 40.0, 2.0].into())
                    .1;
                ui.painter().rect_filled(
                    separator_rect,
                    egui::Rounding::same(1.0),
                    egui::Color32::from_rgba_premultiplied(138, 43, 226, 100),
                );
            });

            ui.add_space(20.0);

            // Mode Selection
            egui::Frame::none()
                .fill(theme.surface)
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("🎯 Mode Selection:")
                            .size(16.0)
                            .color(theme.accent),
                    );
                    ui.add_space(10.0);

                    let modes = [
                        (MetronomeMode::Standard, "🎵", "Standard"),
                        (MetronomeMode::Random, "🎲", "Random"),
                        (MetronomeMode::Practice, "🎯", "Practice"),
                        (MetronomeMode::Polyrhythm, "🔄", "Polyrhythm"),
                        (MetronomeMode::Ritardando, "🐌", "Ritardando"),
                        (MetronomeMode::Subdivision, "🎼", "Subdivision"),
                        (MetronomeMode::Countdown, "⏱️", "Countdown"),
                    ];

                    ui.horizontal_wrapped(|ui| {
                        for (mode, icon, name) in modes.iter() {
                            let selected = *mode == current_mode;
                            let button_color = if selected {
                                match mode {
                                    MetronomeMode::Random => theme.warning,
                                    MetronomeMode::Practice => theme.practice,
                                    MetronomeMode::Polyrhythm => theme.polyrhythm,
                                    MetronomeMode::Ritardando => theme.error,
                                    MetronomeMode::Countdown => theme.countdown,
                                    _ => theme.primary,
                                }
                            } else {
                                theme.surface
                            };

                            if ui
                                .add_sized(
                                    [100.0, 35.0],
                                    egui::Button::new(
                                        egui::RichText::new(format!("{} {}", icon, name)).size(11.0),
                                    )
                                    .fill(button_color)
                                    .rounding(egui::Rounding::same(8.0)),
                                )
                                .clicked()
                            {
                                let _ = self.command_sender.send(MetronomeCommand::ChangeMode(*mode));
                            }
                        }
                    });
                });

            ui.add_space(20.0);

            // Mode-specific controls
            match current_mode {
                MetronomeMode::Random => self.draw_random_controls(ui, &theme),
                MetronomeMode::Practice => self.draw_practice_controls(ui, &theme),
                MetronomeMode::Polyrhythm => self.draw_polyrhythm_controls(ui, &theme),
                MetronomeMode::Ritardando => self.draw_ritardando_controls(ui, &theme),
                MetronomeMode::Subdivision => self.draw_subdivision_controls(ui, &theme),
                MetronomeMode::Countdown => self.draw_countdown_controls(ui, &theme),
                _ => {},
            }

            ui.add_space(20.0);

            // Main metronome display
            ui.vertical_centered(|ui| {
                let base_size = 120.0;
                let max_size = base_size + 40.0;
                let pulse_size = if self.animation_progress > 0.0 {
                    base_size + self.animation_progress * 40.0
                } else {
                    base_size
                };

                // Add celebration glow effect
                let celebration_glow = if self.celebration_animation > 0.0 {
                    self.celebration_animation * 50.0
                } else {
                    0.0
                };

                let beat_color = if is_running {
                    if self.animation_progress > 0.0 || self.celebration_animation > 0.0 {
                        let intensity = if self.celebration_animation > 0.0 {
                            self.celebration_animation
                        } else {
                            0.3 + self.animation_progress * 0.7
                        };
                        match current_mode {
                            MetronomeMode::Random => theme.warning,
                            MetronomeMode::Practice => theme.practice,
                            MetronomeMode::Polyrhythm => theme.polyrhythm,
                            MetronomeMode::Countdown => if self.celebration_animation > 0.0 {
                                egui::Color32::from_rgb(255, 215, 0) // Gold for celebration
                            } else {
                                theme.countdown
                            },
                            _ => egui::Color32::from_rgb(
                                (138.0 + (255.0 - 138.0) * intensity) as u8,
                                (43.0 + (255.0 - 43.0) * intensity) as u8,
                                (226.0 + (255.0 - 226.0) * intensity) as u8,
                            ),
                        }
                    } else {
                        match current_mode {
                            MetronomeMode::Random => theme.warning,
                            MetronomeMode::Practice => theme.practice,
                            MetronomeMode::Polyrhythm => theme.polyrhythm,
                            MetronomeMode::Countdown => theme.countdown,
                            _ => theme.primary,
                        }
                    }
                } else {
                    egui::Color32::from_gray(80)
                };

                let fixed_size = max_size + 40.0 + celebration_glow;
                let (rect, _) =
                    ui.allocate_exact_size([fixed_size, fixed_size].into(), egui::Sense::hover());

                // Draw celebration effects
                if self.celebration_animation > 0.0 {
                    for i in 0..8 {
                        let angle = (i as f32 * PI * 2.0 / 8.0) + (self.celebration_time.elapsed().as_secs_f32() * 2.0);
                        let radius = pulse_size / 2.0 + 30.0 + (self.celebration_animation * 20.0);
                        let star_pos = rect.center() + egui::Vec2::new(
                            angle.cos() * radius,
                            angle.sin() * radius,
                        );
                        ui.painter().text(
                            star_pos,
                            egui::Align2::CENTER_CENTER,
                            "⭐",
                            egui::FontId::proportional(20.0 * self.celebration_animation),
                            egui::Color32::from_rgb(255, 215, 0),
                        );
                    }
                }

                if (is_running && self.animation_progress > 0.0) || self.celebration_animation > 0.0 {
                    let glow_radius = pulse_size / 2.0 + 15.0 + celebration_glow;
                    let glow_alpha = if self.celebration_animation > 0.0 {
                        (self.celebration_animation * 100.0) as u8
                    } else {
                        (self.animation_progress * 50.0) as u8
                    };
                    ui.painter().circle_filled(
                        rect.center(),
                        glow_radius,
                        egui::Color32::from_rgba_premultiplied(
                            beat_color.r(),
                            beat_color.g(),
                            beat_color.b(),
                            glow_alpha,
                        ),
                    );
                }

                ui.painter()
                    .circle_filled(rect.center(), pulse_size / 2.0, beat_color);

                let highlight_color = egui::Color32::from_rgba_premultiplied(255, 255, 255, 60);
                ui.painter()
                    .circle_filled(rect.center(), pulse_size / 2.0 - 5.0, highlight_color);

                let symbol_size = if self.animation_progress > 0.0 {
                    40.0 + self.animation_progress * 8.0
                } else if self.celebration_animation > 0.0 {
                    40.0 + self.celebration_animation * 15.0
                } else {
                    40.0
                };
                let symbol = match current_mode {
                    MetronomeMode::Random => "🎲",
                    MetronomeMode::Practice => "🎯",
                    MetronomeMode::Polyrhythm => "🔄",
                    MetronomeMode::Ritardando => "🐌",
                    MetronomeMode::Subdivision => "🎼",
                    MetronomeMode::Countdown => if self.celebration_animation > 0.0 { "🎉" } else { "⏱️" },
                    _ => "♪",
                };
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    symbol,
                    egui::FontId::proportional(symbol_size),
                    egui::Color32::WHITE,
                );

                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new(format!("{} BPM", bpm))
                        .size(24.0)
                        .color(theme.on_surface)
                        .strong(),
                );
            });

            ui.add_space(20.0);

            // Beat progress bar or countdown progress
            ui.vertical_centered(|ui| {
                if current_mode == MetronomeMode::Countdown {
                    self.draw_countdown_progress(ui, &theme);
                } else {
                    ui.label(
                        egui::RichText::new("Beat Progress")
                            .size(14.0)
                            .color(theme.accent),
                    );
                    ui.add_space(5.0);

                    let slider_width = 400.0;
                    let slider_height = 12.0;
                    let slider_rect = ui
                        .allocate_space([slider_width, slider_height + 20.0].into())
                        .1;

                    let track_rect = egui::Rect::from_center_size(
                        slider_rect.center(),
                        egui::Vec2::new(slider_width, slider_height),
                    );
                    ui.painter().rect_filled(
                        track_rect,
                        egui::Rounding::same(slider_height / 2.0),
                        egui::Color32::from_gray(40),
                    );

                    let progress_width = slider_width * self.beat_progress;
                    let progress_rect = egui::Rect::from_min_size(
                        track_rect.min,
                        egui::Vec2::new(progress_width, slider_height),
                    );

                    let progress_color = if is_running {
                        if self.animation_progress > 0.5 {
                            egui::Color32::from_rgb(255, 255, 255)
                        } else {
                            match current_mode {
                                MetronomeMode::Random => theme.warning,
                                MetronomeMode::Practice => theme.practice,
                                MetronomeMode::Polyrhythm => theme.polyrhythm,
                                MetronomeMode::Countdown => theme.countdown,
                                _ => theme.primary,
                            }
                        }
                    } else {
                        egui::Color32::from_gray(60)
                    };

                    ui.painter().rect_filled(
                        progress_rect,
                        egui::Rounding::same(slider_height / 2.0),
                        progress_color,
                    );

                    // Subdivision marks for subdivision mode
                    if current_mode == MetronomeMode::Subdivision {
                        if let Ok(subdivision_state) = self.shared_state.subdivision_state.try_read() {
                            let subdivisions = subdivision_state.subdivisions;
                            for i in 1..subdivisions {
                                let tick_x = track_rect.min.x + (slider_width * i as f32) / subdivisions as f32;
                                let tick_top = track_rect.min.y - 3.0;
                                let tick_bottom = track_rect.max.y + 3.0;

                                ui.painter().line_segment(
                                    [
                                        egui::pos2(tick_x, tick_top),
                                        egui::pos2(tick_x, tick_bottom),
                                    ],
                                    egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
                                );
                            }
                        }
                    } else {
                        let num_subdivisions = 4;
                        for i in 1..num_subdivisions {
                            let tick_x =
                                track_rect.min.x + (slider_width * i as f32) / num_subdivisions as f32;
                            let tick_top = track_rect.min.y - 3.0;
                            let tick_bottom = track_rect.max.y + 3.0;

                            ui.painter().line_segment(
                                [
                                    egui::pos2(tick_x, tick_top),
                                    egui::pos2(tick_x, tick_bottom),
                                ],
                                egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
                            );
                        }
                    }

                    if is_running {
                        let effective_bpm = match current_mode {
                            MetronomeMode::Subdivision => {
                                if let Ok(subdivision_state) = self.shared_state.subdivision_state.try_read() {
                                    let multiplier = match subdivision_state.subdivisions {
                                        1 => 1.0, 2 => 2.0, 3 => 3.0, 4 => 4.0, _ => 1.0,
                                    };
                                    bpm as f32 * multiplier
                                } else {
                                    bpm as f32
                                }
                            },
                            _ => bpm as f32,
                        };
                        let time_to_next_beat = (60000.0 / effective_bpm.max(1.0)) * (1.0 - self.beat_progress);
                        ui.add_space(15.0);
                        ui.label(
                            egui::RichText::new(format!("Next beat in: {:.1}ms", time_to_next_beat))
                                .size(12.0)
                                .color(theme.accent),
                        );
                    }
                }
            });

            ui.add_space(30.0);

            // Basic controls
            egui::Frame::none()
                .fill(theme.surface)
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(20.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("🎵 Tempo:")
                                .size(16.0)
                                .color(theme.accent),
                        );
                        ui.add_space(20.0);
                        let mut bpm_value = bpm as f32;
                        let slider = egui::Slider::new(&mut bpm_value, 30.0..=300.0)
                            .show_value(false)
                            .handle_shape(egui::style::HandleShape::Circle);
                        if ui.add_sized([250.0, 25.0], slider).changed() {
                            let _ = self.command_sender.send(MetronomeCommand::ChangeBpm(bpm_value as u32));
                        }
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!("{}", bpm))
                                .size(16.0)
                                .color(theme.primary)
                                .strong(),
                        );
                    });

                    ui.add_space(15.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("🔊 Volume:")
                                .size(16.0)
                                .color(theme.accent),
                        );
                        ui.add_space(10.0);
                        let mut volume_value = volume as f32;
                        let slider = egui::Slider::new(&mut volume_value, 0.0..=100.0)
                            .show_value(false)
                            .handle_shape(egui::style::HandleShape::Circle);
                        if ui.add_sized([250.0, 25.0], slider).changed() {
                            let _ = self.command_sender.send(MetronomeCommand::ChangeVolume(volume_value as u32));
                        }
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!("{}%", volume))
                                .size(16.0)
                                .color(theme.primary)
                                .strong(),
                        );
                    });
                });

            ui.add_space(25.0);

            // Start/Stop button
            ui.vertical_centered(|ui| {
                let button_text = if is_running {
                    "⏹️  STOP"
                } else {
                    "▶️  START"
                };
                let button_color = if is_running {
                    theme.error
                } else {
                    theme.success
                };

                if ui
                    .add_sized(
                        [200.0, 50.0],
                        egui::Button::new(egui::RichText::new(button_text).size(18.0).strong())
                            .fill(button_color)
                            .rounding(egui::Rounding::same(25.0)),
                    )
                    .clicked()
                {
                    if is_running {
                        let _ = self.command_sender.send(MetronomeCommand::Stop);
                    } else {
                        let _ = self.command_sender.send(MetronomeCommand::Start);
                    }
                }
            });

            ui.add_space(25.0);

            // Sound Selection
            egui::Frame::none()
                .fill(theme.surface)
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("🎵 Sound Selection:")
                            .size(16.0)
                            .color(theme.accent),
                    );
                    ui.add_space(10.0);

                    let sounds = [
                        ("🔔", "Beep"),
                        ("🥁", "Kick"),
                        ("🖱️", "Click"),
                        ("🔔", "Cowbell"),
                        ("🎺", "Hi-hat"),
                        ("🪵", "Woodblock"),
                        ("🔺", "Triangle"),
                        ("⬜", "Square"),
                    ];
                    let current_sound = self.shared_state.sound_type.load(Ordering::Relaxed) as usize;

                    ui.horizontal_wrapped(|ui| {
                        for (i, (icon, name)) in sounds.iter().enumerate() {
                            let selected = i == current_sound;
                            let button_color = if selected {
                                theme.primary
                            } else {
                                theme.surface
                            };
                            let text_color = if selected {
                                egui::Color32::WHITE
                            } else {
                                theme.on_surface
                            };

                            if ui
                                .add_sized(
                                    [80.0, 35.0],
                                    egui::Button::new(
                                        egui::RichText::new(format!("{}\n{}", icon, name))
                                            .size(10.0)
                                            .color(text_color),
                                    )
                                    .fill(button_color)
                                    .rounding(egui::Rounding::same(8.0)),
                                )
                                .clicked()
                            {
                                let _ = self.command_sender.send(MetronomeCommand::ChangeSoundType(i as u32));
                            }
                        }
                    });
                });

            ui.add_space(20.0);

            // Status display
            let mode_info = self.get_mode_info(current_mode);

            egui::Frame::none()
                .fill(theme.surface)
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(15.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let status_color = if is_running {
                            theme.success
                        } else {
                            theme.error
                        };
                        let status_icon = if is_running { "🟢" } else { "🔴" };
                        let status_text = if is_running {
                            format!("PLAYING - Beat #{} - {}", tick_count, mode_info)
                        } else {
                            format!("STOPPED - {}", mode_info)
                        };

                        ui.label(
                            egui::RichText::new(format!("{} {}", status_icon, status_text))
                                .size(14.0)
                                .color(status_color)
                                .strong(),
                        );
                    });
                });

            ui.add_space(10.0);
                });
        });
    }
}

impl MetronomeApp {
    fn get_mode_info(&self, current_mode: MetronomeMode) -> String {
        match current_mode {
            MetronomeMode::Random => {
                if let Ok(random_state) = self.shared_state.random_state.try_read() {
                    format!("Random Mode - Next change in {} beats", random_state.remaining_ticks)
                } else {
                    "Random Mode".to_string()
                }
            },
            MetronomeMode::Practice => {
                if let Ok(practice_state) = self.shared_state.practice_state.try_read() {
                    format!("Practice Mode - Section {} - {} beats remaining", 
                           practice_state.current_section + 1, 
                           practice_state.section_remaining)
                } else {
                    "Practice Mode".to_string()
                }
            },
            MetronomeMode::Polyrhythm => {
                if let Ok(poly_state) = self.shared_state.polyrhythm_state.try_read() {
                    format!("Polyrhythm Mode - {}:{}", poly_state.primary, poly_state.secondary)
                } else {
                    "Polyrhythm Mode".to_string()
                }
            },
            MetronomeMode::Ritardando => {
                if let Ok(ritardando_state) = self.shared_state.ritardando_state.try_read() {
                    format!("Ritardando - {} beats to {}BPM", 
                           ritardando_state.remaining, 
                           ritardando_state.target_bpm)
                } else {
                    "Ritardando Mode".to_string()
                }
            },
            MetronomeMode::Subdivision => {
                if let Ok(subdivision_state) = self.shared_state.subdivision_state.try_read() {
                    let sub_name = match subdivision_state.subdivisions {
                        1 => "Quarter notes",
                        2 => "Eighth notes", 
                        3 => "Triplets",
                        4 => "Sixteenth notes",
                        _ => "Custom",
                    };
                    format!("Subdivision Mode - {}", sub_name)
                } else {
                    "Subdivision Mode".to_string()
                }
            },
            MetronomeMode::Countdown => {
                if let Ok(countdown_state) = self.shared_state.countdown_state.try_read() {
                    let minutes = (countdown_state.remaining_seconds / 60.0) as u32;
                    let seconds = (countdown_state.remaining_seconds % 60.0) as u32;
                    format!("Countdown Mode - {}:{:02} remaining", minutes, seconds)
                } else {
                    "Countdown Mode".to_string()
                }
            },
            MetronomeMode::Standard => "Standard Mode".to_string(),
        }
    }

    fn draw_countdown_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        if let Ok(countdown_state) = self.shared_state.countdown_state.try_read() {
            let mut duration_seconds = countdown_state.duration_seconds;
            let mut enable_random_bpm = countdown_state.enable_random_bpm;
            let mut changed = false;
            
            egui::Frame::none()
                .fill(theme.countdown.gamma_multiply(0.2))
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .stroke(egui::Stroke::new(2.0, theme.countdown))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("⏱️ Countdown Mode Settings")
                            .size(16.0)
                            .color(theme.countdown)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        ui.label("Duration:");
                        let mut duration_minutes = duration_seconds as f32 / 60.0;
                        if ui.add(egui::Slider::new(&mut duration_minutes, 0.5..=30.0)
                            .suffix(" min")).changed() {
                            duration_seconds = (duration_minutes * 60.0) as u32;
                            changed = true;
                        }
                    });
                    
                    ui.add_space(10.0);
                    
                    if ui.checkbox(&mut enable_random_bpm, "🎲 Randomize BPM during countdown").changed() {
                        changed = true;
                    }
                    
                    if enable_random_bpm {
                        ui.add_space(5.0);
                        ui.label(
                            egui::RichText::new("💡 BPM will randomly change every 3-8 seconds")
                                .size(12.0)
                                .color(theme.countdown),
                        );
                    }
                    
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("🎉 A celebration sound will play when countdown completes!")
                            .size(12.0)
                            .color(theme.countdown),
                    );
                });
                
            if changed {
                let _ = self.command_sender.send(MetronomeCommand::UpdateCountdownSettings {
                    duration_seconds,
                    enable_random_bpm,
                });
            }
        }
    }

    fn draw_countdown_progress(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        if let Ok(countdown_state) = self.shared_state.countdown_state.try_read() {
            ui.label(
                egui::RichText::new("⏱️ Countdown Progress")
                    .size(14.0)
                    .color(theme.countdown),
            );
            ui.add_space(5.0);

            let slider_width = 400.0;
            let slider_height = 20.0;
            let slider_rect = ui
                .allocate_space([slider_width, slider_height + 20.0].into())
                .1;

            let track_rect = egui::Rect::from_center_size(
                slider_rect.center(),
                egui::Vec2::new(slider_width, slider_height),
            );
            
            // Background
            ui.painter().rect_filled(
                track_rect,
                egui::Rounding::same(slider_height / 2.0),
                egui::Color32::from_gray(40),
            );

            // Progress fill
            let progress = if countdown_state.duration_seconds > 0 {
                1.0 - (countdown_state.remaining_seconds / countdown_state.duration_seconds as f32)
            } else {
                0.0
            };
            
            let progress_width = slider_width * progress;
            let progress_rect = egui::Rect::from_min_size(
                track_rect.min,
                egui::Vec2::new(progress_width, slider_height),
            );

            let progress_color = if countdown_state.remaining_seconds <= 10.0 {
                theme.error // Red when less than 10 seconds
            } else if countdown_state.remaining_seconds <= 30.0 {
                theme.warning // Yellow when less than 30 seconds
            } else {
                theme.countdown // Orange otherwise
            };

            ui.painter().rect_filled(
                progress_rect,
                egui::Rounding::same(slider_height / 2.0),
                progress_color,
            );

            // Time display
            let minutes = (countdown_state.remaining_seconds / 60.0) as u32;
            let seconds = (countdown_state.remaining_seconds % 60.0) as u32;
            
            ui.painter().text(
                track_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}:{:02}", minutes, seconds),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );

            ui.add_space(15.0);
            
            if countdown_state.enable_random_bpm {
                ui.label(
                    egui::RichText::new("🎲 Random BPM mode active")
                        .size(12.0)
                        .color(theme.countdown),
                );
            }
        }
    }

    fn draw_random_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        if let Ok(random_state) = self.shared_state.random_state.try_read() {
            let is_running = self.shared_state.is_running.load(Ordering::Relaxed);
            
            egui::Frame::none()
                .fill(theme.warning.gamma_multiply(0.2))
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .stroke(egui::Stroke::new(2.0, theme.warning))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("🎲 Random Mode Settings")
                            .size(16.0)
                            .color(theme.warning)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        ui.label("Change every:");
                        let mut random_count_value = random_state.count as f32;
                        let slider = egui::Slider::new(&mut random_count_value, 10.0..=500.0)
                            .suffix(" beats");
                        if ui.add_sized([200.0, 20.0], slider).changed() {
                            let _ = self.command_sender.send(MetronomeCommand::UpdateRandomSettings {
                                count: random_count_value as u32,
                            });
                        }
                    });
                    
                    if is_running {
                        ui.add_space(10.0);
                        let progress = if random_state.count > 0 {
                            (random_state.count - random_state.remaining_ticks) as f32 / random_state.count as f32
                        } else {
                            0.0
                        };
                        
                        ui.horizontal(|ui| {
                            ui.label(format!("Next change in: {} beats", random_state.remaining_ticks));
                            let progress_bar_width = 150.0;
                            let progress_rect = ui.allocate_space([progress_bar_width, 8.0].into()).1;
                            
                            ui.painter().rect_filled(
                                progress_rect,
                                egui::Rounding::same(4.0),
                                egui::Color32::from_gray(40),
                            );
                            
                            let fill_width = progress_rect.width() * progress;
                            let fill_rect = egui::Rect::from_min_size(
                                progress_rect.min,
                                egui::Vec2::new(fill_width, progress_rect.height()),
                            );
                            
                            ui.painter().rect_filled(
                                fill_rect,
                                egui::Rounding::same(4.0),
                                theme.warning,
                            );
                        });
                    }
                    
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("🎯 BPM will randomly change between 60-200")
                            .size(12.0)
                            .color(theme.warning),
                    );
                });
        }
    }
    
    fn draw_practice_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        if let Ok(mut practice_state) = self.shared_state.practice_state.try_write() {
            egui::Frame::none()
                .fill(theme.practice.gamma_multiply(0.2))
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .stroke(egui::Stroke::new(2.0, theme.practice))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("🎯 Practice Mode Settings")
                            .size(16.0)
                            .color(theme.practice)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    
                    ui.label("Practice sections (BPM, Beats):");
                    
                    let mut to_remove = None;
                    let mut sections_changed = false;
                    
                    for (i, (bpm, beats)) in practice_state.sections.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("Section {}:", i + 1));
                            
                            let mut bpm_f = *bpm as f32;
                            if ui.add(egui::Slider::new(&mut bpm_f, 30.0..=300.0)
                                .suffix(" BPM")).changed() {
                                *bpm = bpm_f as u32;
                                sections_changed = true;
                            }
                            
                            let mut beats_f = *beats as f32;
                            if ui.add(egui::Slider::new(&mut beats_f, 4.0..=128.0)
                                .suffix(" beats")).changed() {
                                *beats = beats_f as u32;
                                sections_changed = true;
                            }
                            
                            if ui.button("❌").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                    
                    if let Some(index) = to_remove {
                        practice_state.sections.remove(index);
                        sections_changed = true;
                    }
                    
                    ui.add_space(10.0);
                    if ui.button("➕ Add Section").clicked() {
                        practice_state.sections.push((120, 32));
                        sections_changed = true;
                    }
                    
                    if sections_changed {
                        let _ = self.command_sender.send(MetronomeCommand::UpdatePracticeSettings {
                            sections: practice_state.sections.clone(),
                        });
                    }
                    
                    let is_running = self.shared_state.is_running.load(Ordering::Relaxed);
                    if is_running {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "Current: Section {} - {} beats remaining", 
                                practice_state.current_section + 1, 
                                practice_state.section_remaining
                            ))
                            .color(theme.practice),
                        );
                    }
                });
        }
    }
    
    fn draw_polyrhythm_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        if let Ok(poly_state) = self.shared_state.polyrhythm_state.try_read() {
            let mut primary = poly_state.primary;
            let mut secondary = poly_state.secondary;
            let mut accent_primary = poly_state.accent_primary;
            let mut accent_secondary = poly_state.accent_secondary;
            let mut changed = false;
            
            egui::Frame::none()
                .fill(theme.polyrhythm.gamma_multiply(0.2))
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .stroke(egui::Stroke::new(2.0, theme.polyrhythm))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("🔄 Polyrhythm Mode Settings")
                            .size(16.0)
                            .color(theme.polyrhythm)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        ui.label("Primary rhythm:");
                        let mut primary_f = primary as f32;
                        if ui.add(egui::Slider::new(&mut primary_f, 2.0..=16.0)).changed() {
                            primary = primary_f as u32;
                            changed = true;
                        }
                        
                        if ui.checkbox(&mut accent_primary, "Accent").changed() {
                            changed = true;
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Secondary rhythm:");
                        let mut secondary_f = secondary as f32;
                        if ui.add(egui::Slider::new(&mut secondary_f, 2.0..=16.0)).changed() {
                            secondary = secondary_f as u32;
                            changed = true;
                        }
                        
                        if ui.checkbox(&mut accent_secondary, "Accent").changed() {
                            changed = true;
                        }
                    });
                    
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("💡 Creates overlapping rhythmic patterns")
                            .size(12.0)
                            .color(theme.polyrhythm),
                    );
                });
                
            if changed {
                let _ = self.command_sender.send(MetronomeCommand::UpdatePolyrhythmSettings {
                    primary,
                    secondary,
                    accent_primary,
                    accent_secondary,
                });
            }
        }
    }
    
    fn draw_ritardando_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        if let Ok(ritardando_state) = self.shared_state.ritardando_state.try_read() {
            let mut start_bpm = ritardando_state.start_bpm;
            let mut target_bpm = ritardando_state.target_bpm;
            let mut duration = ritardando_state.duration;
            let mut changed = false;
            
            egui::Frame::none()
                .fill(theme.error.gamma_multiply(0.2))
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .stroke(egui::Stroke::new(2.0, theme.error))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("🐌 Ritardando Mode Settings")
                            .size(16.0)
                            .color(theme.error)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        ui.label("Start BPM:");
                        let mut start_bpm_f = start_bpm as f32;
                        if ui.add(egui::Slider::new(&mut start_bpm_f, 60.0..=300.0)).changed() {
                            start_bpm = start_bpm_f as u32;
                            changed = true;
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Target BPM:");
                        let mut target_bpm_f = target_bpm as f32;
                        if ui.add(egui::Slider::new(&mut target_bpm_f, 30.0..=250.0)).changed() {
                            target_bpm = target_bpm_f as u32;
                            changed = true;
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Duration:");
                        let mut duration_f = duration as f32;
                        if ui.add(egui::Slider::new(&mut duration_f, 1.0..=256.0).suffix(" beats")).changed() {
                            duration = (duration_f as u32).max(1);
                            changed = true;
                        }
                    });
                    
                    if self.shared_state.is_running.load(Ordering::Relaxed) {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(format!("Slowing down... {} beats remaining", ritardando_state.remaining))
                                .color(theme.error),
                        );
                    }
                });
                
            if changed {
                let _ = self.command_sender.send(MetronomeCommand::UpdateRitardandoSettings {
                    start_bpm,
                    target_bpm,
                    duration,
                });
            }
        }
    }
    
    fn draw_subdivision_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        if let Ok(subdivision_state) = self.shared_state.subdivision_state.try_read() {
            let mut subdivisions = subdivision_state.subdivisions;
            let mut pattern = subdivision_state.accent_pattern.clone();
            let mut changed = false;
            
            egui::Frame::none()
                .fill(theme.primary.gamma_multiply(0.2))
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .stroke(egui::Stroke::new(2.0, theme.primary))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("🎼 Subdivision Mode Settings")
                            .size(16.0)
                            .color(theme.primary)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    
                    ui.horizontal(|ui| {
                        ui.label("Subdivision:");
                        
                        let subdivision_options = [(1, "Quarter"), (2, "Eighth"), (3, "Triplet"), (4, "Sixteenth")];
                        for (value, name) in subdivision_options.iter() {
                            let selected = subdivisions == *value;
                            let button_color = if selected { theme.primary } else { theme.surface };
                            
                            if ui.add_sized([80.0, 25.0], 
                                egui::Button::new(*name).fill(button_color)).clicked() {
                                subdivisions = *value;
                                changed = true;
                            }
                        }
                    });
                    
                    ui.add_space(10.0);
                    ui.label("Accent Pattern:");
                    
                    // Resize pattern if needed
                    if pattern.len() != subdivisions as usize {
                        pattern.resize(subdivisions as usize, false);
                        if subdivisions > 0 {
                            pattern[0] = true; // Always accent the first beat
                        }
                        changed = true;
                    }
                    
                    ui.horizontal(|ui| {
                        for (i, accent) in pattern.iter_mut().enumerate() {
                            let button_text = if *accent { "💥" } else { "○" };
                            let button_color = if *accent { theme.accent } else { theme.surface };
                            
                            if ui.add_sized([40.0, 30.0], 
                                egui::Button::new(button_text).fill(button_color)).clicked() {
                                *accent = !*accent;
                                changed = true;
                            }
                        }
                    });
                    
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("💡 Click beats to toggle accents")
                            .size(12.0)
                            .color(theme.primary),
                    );
                });
                
            if changed {
                let _ = self.command_sender.send(MetronomeCommand::UpdateSubdivisionSettings {
                    subdivisions,
                    pattern,
                });
            }
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 1400.0])
            .with_min_inner_size([600.0, 700.0])
            .with_title("Metronome Studio Pro")
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Metronome Studio Pro",
        options,
        Box::new(|_cc| Ok(Box::new(MetronomeApp::default()))),
    )
}