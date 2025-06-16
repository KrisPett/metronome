use eframe::egui;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU32, Ordering}, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use rodio::{OutputStream, OutputStreamHandle, buffer::SamplesBuffer, Source, Sink};
use rand::Rng;
use std::f32::consts::PI;
use std::collections::HashMap;

struct MetronomeApp {
    state: Arc<MetronomeState>,
    animation_progress: f32,
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    sound_cache: HashMap<u32, Vec<f32>>,
    sink: Arc<Mutex<Sink>>,
}

struct MetronomeState {
    bpm: AtomicU32,
    is_running: AtomicBool,
    volume: AtomicU32,
    sound_type: AtomicU32,
    tick_count: AtomicU32,
    random_mode: AtomicBool,
    random_count: AtomicU32,
    remaining_ticks: AtomicU32,
    last_beat: Arc<Mutex<Instant>>,
}

impl Default for MetronomeApp {
    fn default() -> Self {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Arc::new(Mutex::new(Sink::try_new(&stream_handle).unwrap()));
        
        // Pre-generate all sounds with better quality
        let mut sound_cache = HashMap::new();
        for i in 0..8 {
            let sound_data = match i {
                0 => create_beep_sound(),
                1 => create_kick_sound(),
                2 => create_click_sound(),
                3 => create_cowbell_sound(),
                4 => create_hihat_sound(),
                5 => create_wood_block_sound(),
                6 => create_triangle_sound(),
                7 => create_square_sound(),
                _ => create_beep_sound(),
            };
            sound_cache.insert(i, sound_data);
        }
        
        let state = Arc::new(MetronomeState {
            bpm: AtomicU32::new(120),
            is_running: AtomicBool::new(false),
            volume: AtomicU32::new(80),
            sound_type: AtomicU32::new(0),
            tick_count: AtomicU32::new(0),
            random_mode: AtomicBool::new(false),
            random_count: AtomicU32::new(100),
            remaining_ticks: AtomicU32::new(0),
            last_beat: Arc::new(Mutex::new(Instant::now())),
        });
        
        // Start the timing thread
        let state_clone = Arc::clone(&state);
        let sink_clone = Arc::clone(&sink);
        let sound_cache_clone = sound_cache.clone();
        
        thread::spawn(move || {
            metronome_thread(state_clone, sink_clone, sound_cache_clone);
        });
        
        Self {
            state,
            animation_progress: 0.0,
            _stream,
            stream_handle,
            sound_cache,
            sink,
        }
    }
}

fn metronome_thread(
    state: Arc<MetronomeState>, 
    sink: Arc<Mutex<Sink>>, 
    sound_cache: HashMap<u32, Vec<f32>>
) {
    let mut last_tick = Instant::now();
    
    loop {
        if state.is_running.load(Ordering::Relaxed) {
            let bpm = state.bpm.load(Ordering::Relaxed);
            let beat_interval = Duration::from_millis(60000 / bpm as u64);
            
            if last_tick.elapsed() >= beat_interval {
                // Update tick count
                state.tick_count.fetch_add(1, Ordering::Relaxed);
                
                // Update last beat time for UI animation
                if let Ok(mut last_beat) = state.last_beat.lock() {
                    *last_beat = Instant::now();
                }
                
                // Play sound
                let volume = state.volume.load(Ordering::Relaxed) as f32 / 100.0;
                let sound_type = state.sound_type.load(Ordering::Relaxed);
                
                if let Some(sound_data) = sound_cache.get(&sound_type) {
                    let volume_adjusted_sound: Vec<f32> = sound_data
                        .iter()
                        .map(|&sample| sample * volume)
                        .collect();
                    
                    let source = SamplesBuffer::new(1, 44100, volume_adjusted_sound);
                    
                    // Use sink for better audio management
                    if let Ok(sink_guard) = sink.lock() {
                        sink_guard.append(source);
                    }
                }
                
                // Handle random mode
                if state.random_mode.load(Ordering::Relaxed) {
                    let mut remaining = state.remaining_ticks.load(Ordering::Relaxed);
                    
                    if remaining == 0 {
                        remaining = state.random_count.load(Ordering::Relaxed);
                        state.remaining_ticks.store(remaining, Ordering::Relaxed);
                    }
                    
                    remaining -= 1;
                    state.remaining_ticks.store(remaining, Ordering::Relaxed);
                    
                    if remaining == 0 {
                        let mut rng = rand::thread_rng();
                        let new_bpm = rng.gen_range(60..=200);
                        state.bpm.store(new_bpm, Ordering::Relaxed);
                    }
                }
                
                last_tick = Instant::now();
            }
        } else {
            // Reset timing when stopped
            last_tick = Instant::now();
        }
        
        // Small sleep to prevent excessive CPU usage
        thread::sleep(Duration::from_millis(1));
    }
}

// Improved sound generation functions with consistent quality
fn create_click_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 15;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 40.0).exp();
        let sample = (t * 2000.0 * 2.0 * PI).sin() * envelope * 0.3;
        wave.push(sample);
    }
    wave
}

fn create_wood_block_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 50;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 15.0).exp();

        let freq1 = 1200.0;
        let freq2 = 800.0;
        let sample1 = (t * freq1 * 2.0 * PI).sin() * 0.2;
        let sample2 = (t * freq2 * 2.0 * PI).sin() * 0.15;
        let sample = (sample1 + sample2) * envelope;
        wave.push(sample);
    }
    wave
}

fn create_cowbell_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 80;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 8.0).exp();

        let fundamental = 800.0;
        let sample = ((t * fundamental * 2.0 * PI).sin() * 0.3
            + (t * fundamental * 2.4 * 2.0 * PI).sin() * 0.2
            + (t * fundamental * 3.2 * 2.0 * PI).sin() * 0.1)
            * envelope;
        wave.push(sample);
    }
    wave
}

fn create_kick_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 100;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 10.0).exp();

        let freq = 60.0 * (-t * 8.0).exp();
        let sample = (t * freq * 2.0 * PI).sin() * envelope * 0.4;
        wave.push(sample);
    }
    wave
}

fn create_hihat_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 40;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    
    // Pre-generate noise to avoid timing issues in the audio thread
    let mut noise_samples: Vec<f32> = Vec::with_capacity(samples);
    let mut rng = rand::thread_rng();
    for _ in 0..samples {
        noise_samples.push(rng.gen_range(-1.0..1.0));
    }

    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 25.0).exp();

        let noise = noise_samples[i];
        let filtered_noise = noise * envelope * 0.2;

        let high_freq = (t * 8000.0 * 2.0 * PI).sin() * envelope * 0.05;

        let sample = filtered_noise + high_freq;
        wave.push(sample);
    }
    wave
}

fn create_triangle_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let frequency = 800.0;
    let duration_ms = 60;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let phase = (t * frequency) % 1.0;
        let envelope = (-t * 6.0).exp();

        let sample = if phase < 0.5 {
            4.0 * phase - 1.0
        } else {
            3.0 - 4.0 * phase
        } * 0.2 * envelope;

        wave.push(sample);
    }
    wave
}

fn create_square_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let frequency = 600.0;
    let duration_ms = 50;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let phase = (t * frequency) % 1.0;

        let envelope = (-t * 10.0).exp();
        let sample = if phase < 0.5 { 1.0 } else { -1.0 } * 0.2 * envelope;
        wave.push(sample);
    }
    wave
}

fn create_beep_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let frequency = 800.0;
    let duration_ms = 40;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 6.0).exp();
        let sample = (t * frequency * 2.0 * PI).sin() * 0.2 * envelope;
        wave.push(sample);
    }
    wave
}

impl eframe::App for MetronomeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let bpm = self.state.bpm.load(Ordering::Relaxed);
        let is_running = self.state.is_running.load(Ordering::Relaxed);
        let volume = self.state.volume.load(Ordering::Relaxed);
        let tick_count = self.state.tick_count.load(Ordering::Relaxed);
        let random_mode = self.state.random_mode.load(Ordering::Relaxed);
        let random_count = self.state.random_count.load(Ordering::Relaxed);
        let remaining_ticks = self.state.remaining_ticks.load(Ordering::Relaxed);
        
        // Handle UI animation based on actual beat timing
        if is_running {
            if let Ok(last_beat) = self.state.last_beat.lock() {
                let time_since_beat = last_beat.elapsed().as_millis() as f32;
                let beat_interval_ms = 60000.0 / bpm as f32;
                
                // Create smooth animation pulse
                if time_since_beat < 200.0 {
                    self.animation_progress = 1.0 - (time_since_beat / 200.0);
                } else {
                    self.animation_progress = 0.0;
                }
            }
            ctx.request_repaint();
        } else {
            self.animation_progress = 0.0;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎵 Metronome Studio");
            ui.separator();
            
            // Beat visualization
            let beat_size = if self.animation_progress > 0.0 { 
                80.0 + self.animation_progress * 30.0 
            } else { 
                80.0 
            };
            let beat_color = if is_running { 
                if self.animation_progress > 0.0 {
                    egui::Color32::from_rgb(
                        (255.0 * (0.5 + self.animation_progress * 0.5)) as u8,
                        255,
                        (100.0 * (1.0 - self.animation_progress)) as u8
                    )
                } else {
                    egui::Color32::from_rgb(100, 255, 100)
                }
            } else { 
                egui::Color32::GRAY 
            };
            
            ui.horizontal(|ui| {
                ui.add_space(200.0);
                ui.allocate_ui_with_layout([100.0, 100.0].into(), egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                    let (rect, _) = ui.allocate_exact_size([beat_size, beat_size].into(), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), beat_size / 2.0, beat_color);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "♪",
                        egui::FontId::proportional(30.0),
                        egui::Color32::WHITE,
                    );
                });
            });
            
            ui.add_space(20.0);
            
            // Controls
            ui.horizontal(|ui| {
                ui.label("BPM:");
                let mut bpm_value = bpm as f32;
                if ui.add(egui::Slider::new(&mut bpm_value, 30.0..=300.0)).changed() {
                    self.state.bpm.store(bpm_value as u32, Ordering::Relaxed);
                }
                ui.label(format!("{}", bpm));
            });
            
            ui.horizontal(|ui| {
                ui.label("Volume:");
                let mut volume_value = volume as f32;
                if ui.add(egui::Slider::new(&mut volume_value, 0.0..=100.0)).changed() {
                    self.state.volume.store(volume_value as u32, Ordering::Relaxed);
                }
                ui.label(format!("{}%", volume));
            });
            
            ui.add_space(10.0);
            
            // Random Mode Controls
            ui.horizontal(|ui| {
                let random_button_text = if random_mode { "🎲 Random: ON" } else { "🎯 Random: OFF" };
                let random_button_color = if random_mode { 
                    egui::Color32::from_rgb(255, 165, 0) 
                } else { 
                    egui::Color32::from_rgb(100, 100, 100) 
                };
                
                if ui.add_sized([140.0, 30.0], egui::Button::new(random_button_text).fill(random_button_color)).clicked() {
                    let new_random_mode = !random_mode;
                    self.state.random_mode.store(new_random_mode, Ordering::Relaxed);
                    
                    if new_random_mode {
                        self.state.remaining_ticks.store(random_count, Ordering::Relaxed);
                    } else {
                        self.state.remaining_ticks.store(0, Ordering::Relaxed);
                    }
                }
                
                if random_mode {
                    ui.label("Change every:");
                    let mut random_count_value = random_count as f32;
                    if ui.add(egui::Slider::new(&mut random_count_value, 10.0..=500.0).suffix(" beats")).changed() {
                        let new_count = random_count_value as u32;
                        self.state.random_count.store(new_count, Ordering::Relaxed);
                        
                        if is_running && remaining_ticks > new_count {
                            self.state.remaining_ticks.store(new_count, Ordering::Relaxed);
                        }
                    }
                }
            });
            
            // Random Mode Status
            if random_mode {
                ui.horizontal(|ui| {
                    let countdown_text = if is_running {
                        format!("🎲 Next BPM change in: {} beats (Current: {} BPM)", remaining_ticks, bpm)
                    } else {
                        format!("🎲 Will change BPM every {} beats", random_count)
                    };
                    
                    ui.colored_label(egui::Color32::from_rgb(255, 165, 0), countdown_text);
                });
                
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(200, 200, 200), "Random BPM range: 60-200");
                });
            }
            
            ui.add_space(10.0);
            
            // Start/Stop button
            let button_text = if is_running { "⏹️ Stop" } else { "▶️ Start" };
            let button_color = if is_running { 
                egui::Color32::from_rgb(255, 100, 100) 
            } else { 
                egui::Color32::from_rgb(100, 255, 100) 
            };
            
            if ui.add_sized([120.0, 40.0], egui::Button::new(button_text).fill(button_color)).clicked() {
                let new_state = !is_running;
                self.state.is_running.store(new_state, Ordering::Relaxed);
                if new_state {
                    self.state.tick_count.store(0, Ordering::Relaxed);
                    
                    if random_mode {
                        self.state.remaining_ticks.store(random_count, Ordering::Relaxed);
                    }
                }
            }
            
            ui.add_space(10.0);
            
            // Sound selection
            ui.horizontal(|ui| {
                ui.label("Sound:");
                let sounds = ["🔔 Beep", "🥁 Kick", "🖱️ Click", "🔔 Cowbell", "🎺 Hi-hat", "🪵 Woodblock", "🔺 Triangle", "⬜ Square"];
                let current_sound = self.state.sound_type.load(Ordering::Relaxed) as usize;
                
                for (i, sound) in sounds.iter().enumerate() {
                    let selected = i == current_sound;
                    if ui.selectable_label(selected, *sound).clicked() {
                        self.state.sound_type.store(i as u32, Ordering::Relaxed);
                    }
                }
            });
            
            ui.add_space(20.0);
            
            // Status display
            ui.horizontal(|ui| {
                let status = if is_running { 
                    if random_mode {
                        format!("🟢 Playing (Random Mode) - Beat #{} - {}/4", tick_count, (tick_count % 4) + 1)
                    } else {
                        format!("🟢 Playing - Beat #{} - {}/4", tick_count, (tick_count % 4) + 1)
                    }
                } else { 
                    "🔴 Stopped".to_string() 
                };
                ui.label(status);
            });
            
            // Beat indicator dots with countdown
            ui.horizontal(|ui| {
                ui.label("Beat:");
                for i in 1..=4 {
                    let active = is_running && i <= (tick_count % 4) + 1;
                    let color = if active { egui::Color32::GREEN } else { egui::Color32::GRAY };
                    ui.colored_label(color, "●");
                }
                
                if random_mode && is_running {
                    ui.add_space(20.0);
                    ui.label("|");
                    ui.add_space(10.0);
                    
                    let progress = if random_count > 0 {
                        (random_count - remaining_ticks) as f32 / random_count as f32
                    } else {
                        0.0
                    };
                    
                    let progress_bar_width = 100.0;
                    let progress_rect = ui.allocate_space([progress_bar_width, 20.0].into()).1;
                    
                    ui.painter().rect_filled(
                        progress_rect,
                        egui::Rounding::same(2.0),
                        egui::Color32::from_gray(50),
                    );
                    
                    let fill_width = progress_rect.width() * progress;
                    let fill_rect = egui::Rect::from_min_size(
                        progress_rect.min,
                        egui::Vec2::new(fill_width, progress_rect.height()),
                    );
                    
                    ui.painter().rect_filled(
                        fill_rect,
                        egui::Rounding::same(2.0),
                        egui::Color32::from_rgb(255, 165, 0),
                    );
                    
                    ui.painter().text(
                        progress_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{}", remaining_ticks),
                        egui::FontId::proportional(12.0),
                        egui::Color32::WHITE,
                    );
                }
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 400.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "Metronome Studio",
        options,
        Box::new(|_cc| Ok(Box::new(MetronomeApp::default()))),
    )
}