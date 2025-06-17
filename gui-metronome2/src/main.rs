#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use rand::Rng;
use rodio::{OutputStream, OutputStreamHandle, Sink, buffer::SamplesBuffer};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use std::thread;
use std::time::{Duration, Instant};
mod utilities;
use crate::utilities::sound::{
    create_beep_sound, create_click_sound, create_cowbell_sound, create_hihat_sound,
    create_kick_sound, create_square_sound, create_triangle_sound, create_wood_block_sound,
};

#[derive(Clone, Copy, PartialEq)]
enum MetronomeMode {
    Standard,
    Random,
    Practice,
    Polyrhythm,
    Ritardando,
    Subdivision,
}

struct MetronomeApp {
    state: Arc<MetronomeState>,
    animation_progress: f32,
    beat_progress: f32,
    _stream: OutputStream,
    #[allow(dead_code)]
    stream_handle: OutputStreamHandle,
    #[allow(dead_code)]
    sound_cache: HashMap<u32, Vec<f32>>,
    #[allow(dead_code)]
    sink: Arc<Mutex<Sink>>,
}

struct MetronomeState {
    bpm: AtomicU32,
    is_running: AtomicBool,
    volume: AtomicU32,
    sound_type: AtomicU32,
    tick_count: AtomicU32,
    
    // Mode selection
    mode: Arc<Mutex<MetronomeMode>>,
    
    // Random mode
    random_count: AtomicU32,
    remaining_ticks: AtomicU32,
    
    // Practice mode
    practice_sections: Arc<Mutex<Vec<(u32, u32)>>>, // (BPM, beats)
    current_section: AtomicU32,
    section_remaining: AtomicU32,
    
    // Polyrhythm mode
    poly_primary: AtomicU32,   // Primary rhythm (e.g., 4)
    poly_secondary: AtomicU32, // Secondary rhythm (e.g., 3)
    poly_accent_primary: AtomicBool,
    poly_accent_secondary: AtomicBool,
    
    // Accelerando/Ritardando mode
    start_bpm: AtomicU32,
    target_bpm: AtomicU32,
    tempo_change_duration: AtomicU32, // in beats
    tempo_change_remaining: AtomicU32,
    
    // Subdivision mode
    subdivisions: AtomicU32, // 1=quarter, 2=eighth, 3=triplet, 4=sixteenth
    accent_pattern: Arc<Mutex<Vec<bool>>>,
    
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
            
            mode: Arc::new(Mutex::new(MetronomeMode::Standard)),
            
            random_count: AtomicU32::new(100),
            remaining_ticks: AtomicU32::new(0),
            
            practice_sections: Arc::new(Mutex::new(vec![(60, 32), (120, 32), (180, 32)])),
            current_section: AtomicU32::new(0),
            section_remaining: AtomicU32::new(0),
            
            poly_primary: AtomicU32::new(4),
            poly_secondary: AtomicU32::new(3),
            poly_accent_primary: AtomicBool::new(true),
            poly_accent_secondary: AtomicBool::new(true),
            
            start_bpm: AtomicU32::new(120),
            target_bpm: AtomicU32::new(180),
            tempo_change_duration: AtomicU32::new(64),
            tempo_change_remaining: AtomicU32::new(0),
            
            subdivisions: AtomicU32::new(1),
            accent_pattern: Arc::new(Mutex::new(vec![true, false, false, false])),
            
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
    polyrhythm: egui::Color32,
    practice: egui::Color32,
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
        }
    }
}

fn metronome_thread(
    state: Arc<MetronomeState>,
    sink: Arc<Mutex<Sink>>,
    sound_cache: HashMap<u32, Vec<f32>>,
) {
    let mut last_tick = Instant::now();
    let mut subdivision_tick = 0u32;

    loop {
        if state.is_running.load(Ordering::Relaxed) {
            let current_mode = *state.mode.lock().unwrap();
            let mut effective_bpm = state.bpm.load(Ordering::Relaxed);
            let mut should_tick = false;
            let mut is_accent = false;
            let mut use_alternate_sound = false;

            // Calculate beat interval based on mode
            let beat_interval = match current_mode {
                MetronomeMode::Subdivision => {
                    let subdivisions = state.subdivisions.load(Ordering::Relaxed);
                    let multiplier = match subdivisions {
                        1 => 1.0,  // Quarter notes
                        2 => 2.0,  // Eighth notes
                        3 => 3.0,  // Triplets
                        4 => 4.0,  // Sixteenth notes
                        _ => 1.0,
                    };
                    Duration::from_millis((60000.0 / (effective_bpm as f32 * multiplier)) as u64)
                },
                _ => Duration::from_millis(60000 / effective_bpm as u64),
            };

            if last_tick.elapsed() >= beat_interval {
                should_tick = true;
                
                match current_mode {
                    MetronomeMode::Standard => {
                        // Standard mode - just tick
                    },
                    
                    MetronomeMode::Random => {
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
                    },
                    
                    MetronomeMode::Practice => {
                        let mut section_remaining = state.section_remaining.load(Ordering::Relaxed);
                        if section_remaining == 0 {
                            // Move to next section
                            let sections = state.practice_sections.lock().unwrap();
                            let current_section = state.current_section.load(Ordering::Relaxed) as usize;
                            
                            if current_section < sections.len() {
                                let (section_bpm, section_beats) = sections[current_section];
                                state.bpm.store(section_bpm, Ordering::Relaxed);
                                state.section_remaining.store(section_beats, Ordering::Relaxed);
                                section_remaining = section_beats;
                                
                                // Move to next section for next time
                                let next_section = (current_section + 1) % sections.len();
                                state.current_section.store(next_section as u32, Ordering::Relaxed);
                            }
                        }
                        
                        if section_remaining > 0 {
                            state.section_remaining.store(section_remaining - 1, Ordering::Relaxed);
                        }
                    },
                    
                    MetronomeMode::Polyrhythm => {
                        let primary = state.poly_primary.load(Ordering::Relaxed);
                        let secondary = state.poly_secondary.load(Ordering::Relaxed);
                        let tick_count = state.tick_count.load(Ordering::Relaxed);
                        
                        // Check if this tick aligns with primary rhythm
                        let primary_hit = (tick_count % primary) == 0;
                        // Check if this tick aligns with secondary rhythm  
                        let secondary_hit = (tick_count % secondary) == 0;
                        
                        if primary_hit && state.poly_accent_primary.load(Ordering::Relaxed) {
                            is_accent = true;
                        }
                        if secondary_hit && state.poly_accent_secondary.load(Ordering::Relaxed) {
                            use_alternate_sound = true;
                        }
                    },
                    
                    MetronomeMode::Ritardando => {
                        let mut remaining = state.tempo_change_remaining.load(Ordering::Relaxed);
                        if remaining == 0 {
                            remaining = state.tempo_change_duration.load(Ordering::Relaxed);
                            state.tempo_change_remaining.store(remaining, Ordering::Relaxed);
                        }
                        
                        let start_bpm = state.start_bpm.load(Ordering::Relaxed) as f32;
                        let target_bpm = state.target_bpm.load(Ordering::Relaxed) as f32;
                        let duration = state.tempo_change_duration.load(Ordering::Relaxed) as f32;
                        
                        // Prevent division by zero - ensure duration is at least 1
                        if duration > 0.0 {
                            let progress = (duration - remaining as f32) / duration;
                            let current_bpm = start_bpm - (start_bpm - target_bpm) * progress;
                            state.bpm.store(current_bpm as u32, Ordering::Relaxed);
                        } else {
                            // If duration is 0, just set to target BPM
                            state.bpm.store(target_bpm as u32, Ordering::Relaxed);
                        }
                        
                        if remaining > 0 {
                            state.tempo_change_remaining.store(remaining - 1, Ordering::Relaxed);
                        }
                    },
                    
                    MetronomeMode::Subdivision => {
                        let subdivisions = state.subdivisions.load(Ordering::Relaxed);
                        let accent_pattern = state.accent_pattern.lock().unwrap();
                        
                        let pattern_index = subdivision_tick as usize % accent_pattern.len();
                        is_accent = accent_pattern[pattern_index];
                        
                        subdivision_tick += 1;
                    },
                }

                if should_tick {
                    state.tick_count.fetch_add(1, Ordering::Relaxed);

                    if let Ok(mut last_beat) = state.last_beat.lock() {
                        *last_beat = Instant::now();
                    }

                    let volume = state.volume.load(Ordering::Relaxed) as f32 / 100.0;
                    let mut sound_type = state.sound_type.load(Ordering::Relaxed);
                    
                    // Modify sound based on accent or alternate sound
                    if use_alternate_sound {
                        sound_type = (sound_type + 1) % 8; // Use next sound in list
                    }
                    
                    let final_volume = if is_accent { volume * 1.5 } else { volume };

                    if let Some(sound_data) = sound_cache.get(&sound_type) {
                        let volume_adjusted_sound: Vec<f32> =
                            sound_data.iter().map(|&sample| sample * final_volume).collect();

                        let source = SamplesBuffer::new(1, 44100, volume_adjusted_sound);

                        if let Ok(sink_guard) = sink.lock() {
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
        let current_mode = *self.state.mode.lock().unwrap();

        if is_running {
            if let Ok(last_beat) = self.state.last_beat.lock() {
                let time_since_beat = last_beat.elapsed().as_millis() as f32;
                let effective_bpm = match current_mode {
                    MetronomeMode::Subdivision => {
                        let subdivisions = self.state.subdivisions.load(Ordering::Relaxed);
                        let multiplier = match subdivisions {
                            1 => 1.0, 2 => 2.0, 3 => 3.0, 4 => 4.0, _ => 1.0,
                        };
                        bpm as f32 * multiplier
                    },
                    _ => bpm as f32,
                };
                let beat_interval_ms = 60000.0 / effective_bpm;

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
                                *self.state.mode.lock().unwrap() = *mode;
                                
                                // Reset mode-specific counters
                                match mode {
                                    MetronomeMode::Random => {
                                        self.state.remaining_ticks.store(
                                            self.state.random_count.load(Ordering::Relaxed),
                                            Ordering::Relaxed,
                                        );
                                    },
                                    MetronomeMode::Practice => {
                                        self.state.current_section.store(0, Ordering::Relaxed);
                                        self.state.section_remaining.store(0, Ordering::Relaxed);
                                    },
                                    MetronomeMode::Ritardando => {
                                        self.state.tempo_change_remaining.store(
                                            self.state.tempo_change_duration.load(Ordering::Relaxed),
                                            Ordering::Relaxed,
                                        );
                                    },
                                    _ => {},
                                }
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
                _ => {},
            }

            ui.add_space(20.0);

            // Main metronome display (existing code)
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
                        match current_mode {
                            MetronomeMode::Random => theme.warning,
                            MetronomeMode::Practice => theme.practice,
                            MetronomeMode::Polyrhythm => theme.polyrhythm,
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
                            _ => theme.primary,
                        }
                    }
                } else {
                    egui::Color32::from_gray(80)
                };

                let fixed_size = max_size + 40.0;
                let (rect, _) =
                    ui.allocate_exact_size([fixed_size, fixed_size].into(), egui::Sense::hover());

                if is_running && self.animation_progress > 0.0 {
                    let glow_radius = pulse_size / 2.0 + 15.0;
                    let glow_alpha = (self.animation_progress * 50.0) as u8;
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
                } else {
                    40.0
                };
                let symbol = match current_mode {
                    MetronomeMode::Random => "🎲",
                    MetronomeMode::Practice => "🎯",
                    MetronomeMode::Polyrhythm => "🔄",
                    MetronomeMode::Ritardando => "🐌",
                    MetronomeMode::Subdivision => "🎼",
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

            // Beat progress bar (existing code)
            ui.vertical_centered(|ui| {
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
                    let subdivisions = self.state.subdivisions.load(Ordering::Relaxed);
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
                            let subdivisions = self.state.subdivisions.load(Ordering::Relaxed);
                            let multiplier = match subdivisions {
                                1 => 1.0, 2 => 2.0, 3 => 3.0, 4 => 4.0, _ => 1.0,
                            };
                            bpm as f32 * multiplier
                        },
                        _ => bpm as f32,
                    };
                    let time_to_next_beat = (60000.0 / effective_bpm) * (1.0 - self.beat_progress);
                    ui.add_space(15.0);
                    ui.label(
                        egui::RichText::new(format!("Next beat in: {:.1}ms", time_to_next_beat))
                            .size(12.0)
                            .color(theme.accent),
                    );
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
                            self.state.bpm.store(bpm_value as u32, Ordering::Relaxed);
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
                            self.state
                                .volume
                                .store(volume_value as u32, Ordering::Relaxed);
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
                    let new_state = !is_running;
                    self.state.is_running.store(new_state, Ordering::Relaxed);
                    if new_state {
                        self.state.tick_count.store(0, Ordering::Relaxed);
                        
                        // Reset mode-specific counters
                        match current_mode {
                            MetronomeMode::Random => {
                                self.state.remaining_ticks.store(
                                    self.state.random_count.load(Ordering::Relaxed),
                                    Ordering::Relaxed,
                                );
                            },
                            MetronomeMode::Practice => {
                                self.state.current_section.store(0, Ordering::Relaxed);
                                self.state.section_remaining.store(0, Ordering::Relaxed);
                            },
                            MetronomeMode::Ritardando => {
                                self.state.tempo_change_remaining.store(
                                    self.state.tempo_change_duration.load(Ordering::Relaxed),
                                    Ordering::Relaxed,
                                );
                            },
                            _ => {},
                        }
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
                    let current_sound = self.state.sound_type.load(Ordering::Relaxed) as usize;

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
                                self.state.sound_type.store(i as u32, Ordering::Relaxed);
                            }
                        }
                    });
                });

            ui.add_space(20.0);

            // Status display
            let mode_info = match current_mode {
                MetronomeMode::Random => {
                    let remaining = self.state.remaining_ticks.load(Ordering::Relaxed);
                    format!("Random Mode - Next change in {} beats", remaining)
                },
                MetronomeMode::Practice => {
                    let current_section = self.state.current_section.load(Ordering::Relaxed);
                    let section_remaining = self.state.section_remaining.load(Ordering::Relaxed);
                    format!("Practice Mode - Section {} - {} beats remaining", current_section + 1, section_remaining)
                },
                MetronomeMode::Polyrhythm => {
                    let primary = self.state.poly_primary.load(Ordering::Relaxed);
                    let secondary = self.state.poly_secondary.load(Ordering::Relaxed);
                    format!("Polyrhythm Mode - {}:{}", primary, secondary)
                },
                MetronomeMode::Ritardando => {
                    let remaining = self.state.tempo_change_remaining.load(Ordering::Relaxed);
                    let target = self.state.target_bpm.load(Ordering::Relaxed);
                    format!("Ritardando - {} beats to {}BPM", remaining, target)
                },
                MetronomeMode::Subdivision => {
                    let subdivisions = self.state.subdivisions.load(Ordering::Relaxed);
                    let sub_name = match subdivisions {
                        1 => "Quarter notes",
                        2 => "Eighth notes", 
                        3 => "Triplets",
                        4 => "Sixteenth notes",
                        _ => "Custom",
                    };
                    format!("Subdivision Mode - {}", sub_name)
                },
                MetronomeMode::Standard => "Standard Mode".to_string(),
            };

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
    fn draw_random_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        let random_count = self.state.random_count.load(Ordering::Relaxed);
        let remaining_ticks = self.state.remaining_ticks.load(Ordering::Relaxed);
        let is_running = self.state.is_running.load(Ordering::Relaxed);
        
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
                    let mut random_count_value = random_count as f32;
                    let slider = egui::Slider::new(&mut random_count_value, 10.0..=500.0)
                        .suffix(" beats");
                    if ui.add_sized([200.0, 20.0], slider).changed() {
                        let new_count = random_count_value as u32;
                        self.state.random_count.store(new_count, Ordering::Relaxed);
                        
                        // If currently running and remaining ticks is greater than new count,
                        // reset remaining ticks to new count to avoid issues
                        if is_running && remaining_ticks > new_count {
                            self.state.remaining_ticks.store(new_count, Ordering::Relaxed);
                        }
                    }
                });
                
                if is_running {
                    ui.add_space(10.0);
                    let progress = if random_count > 0 {
                        (random_count - remaining_ticks) as f32 / random_count as f32
                    } else {
                        0.0
                    };
                    
                    ui.horizontal(|ui| {
                        ui.label(format!("Next change in: {} beats", remaining_ticks));
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
    
    fn draw_practice_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
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
                
                if let Ok(mut sections) = self.state.practice_sections.try_lock() {
                    let mut to_remove = None;
                    
                    for (i, (bpm, beats)) in sections.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("Section {}:", i + 1));
                            
                            let mut bpm_f = *bpm as f32;
                            if ui.add(egui::Slider::new(&mut bpm_f, 30.0..=300.0)
                                .suffix(" BPM")).changed() {
                                *bpm = bpm_f as u32;
                            }
                            
                            let mut beats_f = *beats as f32;
                            if ui.add(egui::Slider::new(&mut beats_f, 4.0..=128.0)
                                .suffix(" beats")).changed() {
                                *beats = beats_f as u32;
                            }
                            
                            if ui.button("❌").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                    
                    if let Some(index) = to_remove {
                        sections.remove(index);
                    }
                    
                    ui.add_space(10.0);
                    if ui.button("➕ Add Section").clicked() {
                        sections.push((120, 32));
                    }
                }
                
                let current_section = self.state.current_section.load(Ordering::Relaxed);
                let section_remaining = self.state.section_remaining.load(Ordering::Relaxed);
                
                if self.state.is_running.load(Ordering::Relaxed) {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Current: Section {} - {} beats remaining", 
                            current_section + 1, 
                            section_remaining
                        ))
                        .color(theme.practice),
                    );
                }
            });
    }
    
    fn draw_polyrhythm_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
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
                    let mut primary = self.state.poly_primary.load(Ordering::Relaxed) as f32;
                    if ui.add(egui::Slider::new(&mut primary, 2.0..=16.0)).changed() {
                        self.state.poly_primary.store(primary as u32, Ordering::Relaxed);
                    }
                    
                    let mut accent_primary = self.state.poly_accent_primary.load(Ordering::Relaxed);
                    if ui.checkbox(&mut accent_primary, "Accent").changed() {
                        self.state.poly_accent_primary.store(accent_primary, Ordering::Relaxed);
                    }
                });
                
                ui.horizontal(|ui| {
                    ui.label("Secondary rhythm:");
                    let mut secondary = self.state.poly_secondary.load(Ordering::Relaxed) as f32;
                    if ui.add(egui::Slider::new(&mut secondary, 2.0..=16.0)).changed() {
                        self.state.poly_secondary.store(secondary as u32, Ordering::Relaxed);
                    }
                    
                    let mut accent_secondary = self.state.poly_accent_secondary.load(Ordering::Relaxed);
                    if ui.checkbox(&mut accent_secondary, "Accent").changed() {
                        self.state.poly_accent_secondary.store(accent_secondary, Ordering::Relaxed);
                    }
                });
                
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new("💡 Creates overlapping rhythmic patterns")
                        .size(12.0)
                        .color(theme.polyrhythm),
                );
            });
    }
    
    fn draw_ritardando_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
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
                    let mut start_bpm = self.state.start_bpm.load(Ordering::Relaxed) as f32;
                    if ui.add(egui::Slider::new(&mut start_bpm, 60.0..=300.0)).changed() {
                        self.state.start_bpm.store(start_bpm as u32, Ordering::Relaxed);
                        self.state.bpm.store(start_bpm as u32, Ordering::Relaxed);
                    }
                });
                
                ui.horizontal(|ui| {
                    ui.label("Target BPM:");
                    let mut target_bpm = self.state.target_bpm.load(Ordering::Relaxed) as f32;
                    if ui.add(egui::Slider::new(&mut target_bpm, 30.0..=250.0)).changed() {
                        self.state.target_bpm.store(target_bpm as u32, Ordering::Relaxed);
                    }
                });
                
                ui.horizontal(|ui| {
                    ui.label("Duration:");
                    let mut duration = self.state.tempo_change_duration.load(Ordering::Relaxed) as f32;
                    if ui.add(egui::Slider::new(&mut duration, 1.0..=256.0).suffix(" beats")).changed() {
                        // Ensure minimum value is at least 1 to prevent division by zero
                        let safe_duration = (duration as u32).max(1);
                        self.state.tempo_change_duration.store(safe_duration, Ordering::Relaxed);
                    }
                });
                
                if self.state.is_running.load(Ordering::Relaxed) {
                    let remaining = self.state.tempo_change_remaining.load(Ordering::Relaxed);
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(format!("Slowing down... {} beats remaining", remaining))
                            .color(theme.error),
                    );
                }
            });
    }
    
    fn draw_subdivision_controls(&mut self, ui: &mut egui::Ui, theme: &Theme) {
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
                    let current_sub = self.state.subdivisions.load(Ordering::Relaxed);
                    
                    let subdivisions = [(1, "Quarter"), (2, "Eighth"), (3, "Triplet"), (4, "Sixteenth")];
                    for (value, name) in subdivisions.iter() {
                        let selected = current_sub == *value;
                        let button_color = if selected { theme.primary } else { theme.surface };
                        
                        if ui.add_sized([80.0, 25.0], 
                            egui::Button::new(*name).fill(button_color)).clicked() {
                            self.state.subdivisions.store(*value, Ordering::Relaxed);
                        }
                    }
                });
                
                ui.add_space(10.0);
                ui.label("Accent Pattern:");
                
                if let Ok(mut pattern) = self.state.accent_pattern.try_lock() {
                    let subdivisions = self.state.subdivisions.load(Ordering::Relaxed) as usize;
                    
                    // Resize pattern if needed
                    if pattern.len() != subdivisions {
                        pattern.resize(subdivisions, false);
                        if subdivisions > 0 {
                            pattern[0] = true; // Always accent the first beat
                        }
                    }
                    
                    ui.horizontal(|ui| {
                        for (i, accent) in pattern.iter_mut().enumerate() {
                            let button_text = if *accent { "💥" } else { "○" };
                            let button_color = if *accent { theme.accent } else { theme.surface };
                            
                            if ui.add_sized([40.0, 30.0], 
                                egui::Button::new(button_text).fill(button_color)).clicked() {
                                *accent = !*accent;
                            }
                        }
                    });
                }
                
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new("💡 Click beats to toggle accents")
                        .size(12.0)
                        .color(theme.primary),
                );
            });
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