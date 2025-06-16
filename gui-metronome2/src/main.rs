use eframe::egui;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU32, Ordering}, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use rodio::{OutputStream, OutputStreamHandle, buffer::SamplesBuffer, Sink};
use rand::Rng;
use std::f32::consts::PI;
use std::collections::HashMap;

struct MetronomeApp {
    state: Arc<MetronomeState>,
    animation_progress: f32,
    beat_progress: f32,
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
        
        let state_clone = Arc::clone(&state);
        let sink_clone = Arc::clone(&sink);
        let sound_cache_clone = sound_cache.clone();
        
        thread::spawn(move || {
            metronome_thread(state_clone, sink_clone, sound_cache_clone);
        });
        
        Self {
            state,
            animation_progress: 0.0,
            beat_progress: 0.0,
            _stream,
            stream_handle,
            sound_cache,
            sink,
        }
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
                state.tick_count.fetch_add(1, Ordering::Relaxed);
                
                if let Ok(mut last_beat) = state.last_beat.lock() {
                    *last_beat = Instant::now();
                }
                
                let volume = state.volume.load(Ordering::Relaxed) as f32 / 100.0;
                let sound_type = state.sound_type.load(Ordering::Relaxed);
                
                if let Some(sound_data) = sound_cache.get(&sound_type) {
                    let volume_adjusted_sound: Vec<f32> = sound_data
                        .iter()
                        .map(|&sample| sample * volume)
                        .collect();
                    
                    let source = SamplesBuffer::new(1, 44100, volume_adjusted_sound);
                    
                    if let Ok(sink_guard) = sink.lock() {
                        sink_guard.append(source);
                    }
                }
                
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
            last_tick = Instant::now();
        }
        
        thread::sleep(Duration::from_millis(1));
    }
}

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

pub fn create_kick_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 150;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 12.0).exp();

        let freq = 60.0 * (-t * 10.0).exp();
        let sample = (t * freq * 2.0 * PI).sin() * envelope * 0.6;
        wave.push(sample);
    }
    wave
}

fn create_hihat_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 40;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    
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

        let bpm = self.state.bpm.load(Ordering::Relaxed);
        let is_running = self.state.is_running.load(Ordering::Relaxed);
        let volume = self.state.volume.load(Ordering::Relaxed);
        let tick_count = self.state.tick_count.load(Ordering::Relaxed);
        let random_mode = self.state.random_mode.load(Ordering::Relaxed);
        let random_count = self.state.random_count.load(Ordering::Relaxed);
        let remaining_ticks = self.state.remaining_ticks.load(Ordering::Relaxed);
        
        if is_running {
            if let Ok(last_beat) = self.state.last_beat.lock() {
                let time_since_beat = last_beat.elapsed().as_millis() as f32;
                let beat_interval_ms = 60000.0 / bpm as f32;
                
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
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading(egui::RichText::new("🎵 METRONOME STUDIO")
                    .size(32.0)
                    .color(theme.primary)
                    .strong());
                ui.add_space(10.0);
                
                let separator_rect = ui.allocate_space([ui.available_width() - 40.0, 2.0].into()).1;
                ui.painter().rect_filled(
                    separator_rect,
                    egui::Rounding::same(1.0),
                    egui::Color32::from_rgba_premultiplied(138, 43, 226, 100),
                );
            });
            
            ui.add_space(30.0);
            
            ui.vertical_centered(|ui| {
                let base_size = 120.0;
                let max_size = base_size + 40.0; 
                let pulse_size = if self.animation_progress > 0.0 { 
                    base_size + self.animation_progress * 40.0 
                } else { 
                    base_size 
                };
                
                let beat_color = if is_running { 
                    if self.animation_progress > 0.0 {
                        let intensity = 0.3 + self.animation_progress * 0.7;
                        egui::Color32::from_rgb(
                            (138.0 + (255.0 - 138.0) * intensity) as u8,
                            (43.0 + (255.0 - 43.0) * intensity) as u8,
                            (226.0 + (255.0 - 226.0) * intensity) as u8,
                        )
                    } else {
                        theme.primary
                    }
                } else { 
                    egui::Color32::from_gray(80) 
                };
                
                let fixed_size = max_size + 40.0;
                let (rect, _) = ui.allocate_exact_size([fixed_size, fixed_size].into(), egui::Sense::hover());
                
                if is_running && self.animation_progress > 0.0 {
                    let glow_radius = pulse_size / 2.0 + 15.0;
                    let glow_alpha = (self.animation_progress * 50.0) as u8;
                    ui.painter().circle_filled(
                        rect.center(),
                        glow_radius,
                        egui::Color32::from_rgba_premultiplied(138, 43, 226, glow_alpha),
                    );
                }
                
                ui.painter().circle_filled(rect.center(), pulse_size / 2.0, beat_color);
                
                let highlight_color = egui::Color32::from_rgba_premultiplied(255, 255, 255, 60);
                ui.painter().circle_filled(rect.center(), pulse_size / 2.0 - 5.0, highlight_color);
                
                let symbol_size = if self.animation_progress > 0.0 {
                    40.0 + self.animation_progress * 8.0 
                } else {
                    40.0
                };
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "♪",
                    egui::FontId::proportional(symbol_size),
                    egui::Color32::WHITE,
                );
                
                ui.add_space(20.0);
                ui.label(egui::RichText::new(format!("{} BPM", bpm))
                    .size(24.0)
                    .color(theme.on_surface)
                    .strong());
            });
            
            ui.add_space(20.0);
            
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new("Beat Progress")
                    .size(14.0)
                    .color(theme.accent));
                ui.add_space(5.0);
                
                let slider_width = 400.0;
                let slider_height = 12.0;
                let slider_rect = ui.allocate_space([slider_width, slider_height + 20.0].into()).1;
                
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
                        theme.primary
                    }
                } else {
                    egui::Color32::from_gray(60)
                };
                
                ui.painter().rect_filled(
                    progress_rect,
                    egui::Rounding::same(slider_height / 2.0),
                    progress_color,
                );
                
                let num_subdivisions = 4; 
                for i in 1..num_subdivisions {
                    let tick_x = track_rect.min.x + (slider_width * i as f32) / num_subdivisions as f32;
                    let tick_top = track_rect.min.y - 3.0;
                    let tick_bottom = track_rect.max.y + 3.0;
                    
                    ui.painter().line_segment(
                        [egui::pos2(tick_x, tick_top), egui::pos2(tick_x, tick_bottom)],
                        egui::Stroke::new(1.0, egui::Color32::from_gray(100)),
                    );
                }
                
                let beat_marker_y = track_rect.center().y + 25.0;
                for i in 0..=4 {
                    let marker_x = track_rect.min.x + (slider_width * i as f32) / 4.0;
                    let marker_color = if is_running && i == (tick_count % 4) as usize {
                        egui::Color32::from_rgb(255, 215, 0) 
                    } else {
                        theme.on_surface
                    };
                    
                    ui.painter().text(
                        egui::pos2(marker_x, beat_marker_y),
                        egui::Align2::CENTER_CENTER,
                        "T",
                        egui::FontId::proportional(16.0),
                        marker_color,
                    );
                }
                
                if is_running {
                    let time_to_next_beat = (60000.0 / bpm as f32) * (1.0 - self.beat_progress);
                    ui.add_space(15.0);
                    ui.label(egui::RichText::new(format!("Next beat in: {:.1}ms", time_to_next_beat))
                        .size(12.0)
                        .color(theme.accent));
                }
            });
            
            ui.add_space(30.0);
            
            egui::Frame::none()
                .fill(theme.surface)
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(20.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🎵 Tempo:").size(16.0).color(theme.accent));
                        ui.add_space(20.0);
                        let mut bpm_value = bpm as f32;
                        let slider = egui::Slider::new(&mut bpm_value, 30.0..=300.0)
                            .show_value(false)
                            .handle_shape(egui::style::HandleShape::Circle);
                        if ui.add_sized([250.0, 25.0], slider).changed() {
                            self.state.bpm.store(bpm_value as u32, Ordering::Relaxed);
                        }
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(format!("{}", bpm))
                            .size(16.0)
                            .color(theme.primary)
                            .strong());
                    });
                    
                    ui.add_space(15.0);
                    
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🔊 Volume:").size(16.0).color(theme.accent));
                        ui.add_space(10.0);
                        let mut volume_value = volume as f32;
                        let slider = egui::Slider::new(&mut volume_value, 0.0..=100.0)
                            .show_value(false)
                            .handle_shape(egui::style::HandleShape::Circle);
                        if ui.add_sized([250.0, 25.0], slider).changed() {
                            self.state.volume.store(volume_value as u32, Ordering::Relaxed);
                        }
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(format!("{}%", volume))
                            .size(16.0)
                            .color(theme.primary)
                            .strong());
                    });
                });
            
            ui.add_space(20.0);
            
            egui::Frame::none()
                .fill(if random_mode { theme.warning.gamma_multiply(0.2) } else { theme.surface })
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .stroke(if random_mode { 
                    egui::Stroke::new(2.0, theme.warning) 
                } else { 
                    egui::Stroke::NONE 
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let random_text = if random_mode { "🎲 RANDOM MODE: ON" } else { "🎯 Random Mode: OFF" };
                        let button_color = if random_mode { theme.warning } else { theme.surface };
                        
                        if ui.add_sized([180.0, 35.0], 
                            egui::Button::new(egui::RichText::new(random_text).size(14.0).strong())
                                .fill(button_color)
                                .rounding(egui::Rounding::same(8.0))
                        ).clicked() {
                            let new_random_mode = !random_mode;
                            self.state.random_mode.store(new_random_mode, Ordering::Relaxed);
                            
                            if new_random_mode {
                                self.state.remaining_ticks.store(random_count, Ordering::Relaxed);
                            } else {
                                self.state.remaining_ticks.store(0, Ordering::Relaxed);
                            }
                        }
                        
                        if random_mode {
                            ui.add_space(20.0);
                            ui.label(egui::RichText::new("Change every:").color(theme.warning));
                            let mut random_count_value = random_count as f32;
                            let slider = egui::Slider::new(&mut random_count_value, 10.0..=500.0)
                                .suffix(" beats")
                                .show_value(false);
                            if ui.add_sized([150.0, 20.0], slider).changed() {
                                let new_count = random_count_value as u32;
                                self.state.random_count.store(new_count, Ordering::Relaxed);
                                
                                if is_running && remaining_ticks > new_count {
                                    self.state.remaining_ticks.store(new_count, Ordering::Relaxed);
                                }
                            }
                        }
                    });
                    
                    if random_mode {
                        ui.add_space(10.0);
                        if is_running {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("🎲 Next BPM change in: {} beats", remaining_ticks))
                                    .color(theme.warning));
                                
                                ui.add_space(20.0);
                                
                                let progress = if random_count > 0 {
                                    (random_count - remaining_ticks) as f32 / random_count as f32
                                } else {
                                    0.0
                                };
                                
                                let progress_bar_width = 120.0;
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
                        } else {
                            ui.label(egui::RichText::new("🎲 Will change BPM every {} beats (Range: 60-200)")
                                .color(theme.warning));
                        }
                    }
                });
            
            ui.add_space(25.0);
            
            ui.vertical_centered(|ui| {
                let button_text = if is_running { "⏹️  STOP" } else { "▶️  START" };
                let button_color = if is_running { theme.error } else { theme.success };
                
                if ui.add_sized([200.0, 50.0], 
                    egui::Button::new(egui::RichText::new(button_text).size(18.0).strong())
                        .fill(button_color)
                        .rounding(egui::Rounding::same(25.0))
                ).clicked() {
                    let new_state = !is_running;
                    self.state.is_running.store(new_state, Ordering::Relaxed);
                    if new_state {
                        self.state.tick_count.store(0, Ordering::Relaxed);
                        
                        if random_mode {
                            self.state.remaining_ticks.store(random_count, Ordering::Relaxed);
                        }
                    }
                }
            });
            
            ui.add_space(25.0);
            
            egui::Frame::none()
                .fill(theme.surface)
                .rounding(egui::Rounding::same(12.0))
                .inner_margin(egui::Margin::same(15.0))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("🎵 Sound Selection:").size(16.0).color(theme.accent));
                    ui.add_space(10.0);
                    
                    let sounds = [
                        ("🔔", "Beep"), ("🥁", "Kick"), ("🖱️", "Click"), ("🔔", "Cowbell"),
                        ("🎺", "Hi-hat"), ("🪵", "Woodblock"), ("🔺", "Triangle"), ("⬜", "Square")
                    ];
                    let current_sound = self.state.sound_type.load(Ordering::Relaxed) as usize;
                    
                    ui.horizontal_wrapped(|ui| {
                        for (i, (icon, name)) in sounds.iter().enumerate() {
                            let selected = i == current_sound;
                            let button_color = if selected { theme.primary } else { theme.surface };
                            let text_color = if selected { egui::Color32::WHITE } else { theme.on_surface };
                            
                            if ui.add_sized([80.0, 35.0],
                                egui::Button::new(egui::RichText::new(format!("{}\n{}", icon, name))
                                    .size(10.0)
                                    .color(text_color))
                                    .fill(button_color)
                                    .rounding(egui::Rounding::same(8.0))
                            ).clicked() {
                                self.state.sound_type.store(i as u32, Ordering::Relaxed);
                            }
                        }
                    });
                });
            
            ui.add_space(20.0);
            
            egui::Frame::none()
                .fill(theme.surface)
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(15.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let status_color = if is_running { theme.success } else { theme.error };
                        let status_icon = if is_running { "🟢" } else { "🔴" };
                        let status_text = if is_running { 
                            if random_mode {
                                format!("PLAYING (Random Mode) - Beat #{}", tick_count)
                            } else {
                                format!("PLAYING - Beat #{}", tick_count)
                            }
                        } else { 
                            "STOPPED".to_string() 
                        };
                        
                        ui.label(egui::RichText::new(format!("{} {}", status_icon, status_text))
                            .size(14.0)
                            .color(status_color)
                            .strong());
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("Beat Pattern:").color(theme.on_surface));
                            ui.add_space(10.0);
                            
                            for i in 1..=4 {
                                let active = is_running && i <= (tick_count % 4) + 1;
                                let dot_color = if active { theme.primary } else { egui::Color32::from_gray(60) };
                                let dot_size = if active { 12.0 } else { 8.0 };
                                
                                let (rect, _) = ui.allocate_exact_size([16.0, 16.0].into(), egui::Sense::hover());
                                ui.painter().circle_filled(rect.center(), dot_size / 2.0, dot_color);
                                
                                if active {
                                    ui.painter().circle_filled(
                                        rect.center(), 
                                        dot_size / 2.0 + 2.0, 
                                        egui::Color32::from_rgba_premultiplied(138, 43, 226, 30)
                                    );
                                }
                            }
                        });
                    });
                });
            
            ui.add_space(10.0);
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 1300.0])
            .with_min_inner_size([500.0, 600.0])
            .with_title("Metronome Studio")
            .with_resizable(true),
        ..Default::default()
    };
    
    eframe::run_native(
        "Metronome Studio",
        options,
        Box::new(|_cc| Ok(Box::new(MetronomeApp::default()))),
    )
}