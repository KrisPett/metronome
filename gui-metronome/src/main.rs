use std::io::{self, Write, BufWriter, Stdout};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use rodio::{OutputStream, Sink};
use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor, SetBackgroundColor, Attribute, SetAttribute},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use rand::Rng;
use std::collections::HashMap;

use crate::utilities::sound::{create_beep_sound, create_kick_sound, create_click_sound, create_cowbell_sound, create_hihat_sound, create_square_sound, create_triangle_sound, create_wood_block_sound};
mod utilities;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SoundType {
    Beep,
    Kick,
    Click,
    Cowbell,
    Hihat,
    Square,
    Triangle,
    Woodblock
}

impl Default for SoundType {
    fn default() -> Self {
        SoundType::Kick
    }
}

impl SoundType {
    const ALL: [SoundType; 8] = [
        SoundType::Beep,
        SoundType::Kick,
        SoundType::Click,
        SoundType::Cowbell,
        SoundType::Hihat,
        SoundType::Square,
        SoundType::Triangle,
        SoundType::Woodblock,
    ];

    fn next(&self) -> Self {
        let current_idx = Self::ALL.iter().position(|&s| s == *self).unwrap();
        Self::ALL[(current_idx + 1) % Self::ALL.len()]
    }

    fn prev(&self) -> Self {
        let current_idx = Self::ALL.iter().position(|&s| s == *self).unwrap();
        Self::ALL[(current_idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn name(&self) -> &'static str {
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

    fn icon(&self) -> &'static str {
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

    fn create_sound(&self) -> Vec<f32> {
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

struct AtomicState {
    bpm: AtomicU32,
    is_running: AtomicBool,
    random_mode: AtomicBool,
    random_count: AtomicU32,
    remaining_ticks: AtomicU32,
    sound_type: AtomicU32,
    ui_dirty: AtomicBool,
    last_tick_time: AtomicU64,
    tick_count: AtomicU32,
    volume: AtomicU32, // 0-100
}

impl AtomicState {
    fn new() -> Self {
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

    fn get_sound_type(&self) -> SoundType {
        let index = self.sound_type.load(Ordering::Relaxed) as usize;
        SoundType::ALL[index.min(SoundType::ALL.len() - 1)]
    }

    fn set_sound_type(&self, sound_type: SoundType) {
        if let Some(index) = SoundType::ALL.iter().position(|&s| s == sound_type) {
            self.sound_type.store(index as u32, Ordering::Relaxed);
        }
    }

    fn update_tick(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        self.last_tick_time.store(now, Ordering::Relaxed);
        self.tick_count.fetch_add(1, Ordering::Relaxed);
    }

    fn get_last_tick_elapsed(&self) -> Duration {
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

#[derive(Default)]
struct UICache {
    last_bpm: u32,
    last_sound: SoundType,
    last_status: bool,
    last_random_mode: bool,
    last_remaining_ticks: u32,
    last_random_count: u32,
    last_tick_count: u32,
    last_volume: u32,
    first_render: bool,
    animation_buffer: String,
    last_animation_frame: usize,
}

impl UICache {
    fn new() -> Self {
        Self {
            first_render: true,
            animation_buffer: String::with_capacity(100),
            ..Default::default()
        }
    }
}

enum AudioCommand {
    PlayTick(Vec<f32>),
    Stop,
}

struct SoundCache {
    sounds: HashMap<SoundType, Vec<f32>>,
}

impl SoundCache {
    fn new() -> Self {
        let mut sounds = HashMap::new();
        for &sound_type in &SoundType::ALL {
            sounds.insert(sound_type, sound_type.create_sound());
        }
        Self { sounds }
    }

    fn get_sound(&self, sound_type: SoundType) -> &Vec<f32> {
        &self.sounds[&sound_type]
    }
}

// Enhanced UI Layout Constants
const TITLE_ROW: u16 = 1;
const SUBTITLE_ROW: u16 = 2;
const DIVIDER_ROW: u16 = 3;
const MAIN_PANEL_START: u16 = 5;
const ANIMATION_ROW: u16 = 6;
const BPM_PANEL_ROW: u16 = 8;
const SOUND_PANEL_ROW: u16 = 10;
const STATUS_PANEL_ROW: u16 = 12;
const RANDOM_PANEL_ROW: u16 = 14;
const VOLUME_PANEL_ROW: u16 = 16;
const CONTROLS_SECTION_START: u16 = 19;
const CONTROLS_TITLE_ROW: u16 = 20;
const CONTROLS_START_ROW: u16 = 21;
const SOUNDS_SECTION_ROW: u16 = 32;
const FOOTER_ROW: u16 = 35;

fn draw_box_border(writer: &mut BufWriter<Stdout>, x: u16, y: u16, width: u16, height: u16) -> Result<(), Box<dyn std::error::Error>> {
    // Top border
    execute!(writer, cursor::MoveTo(x, y), Print("+"))?;
    for _ in 1..width-1 {
        execute!(writer, Print("-"))?;
    }
    execute!(writer, Print("+"))?;
    
    // Side borders
    for i in 1..height-1 {
        execute!(writer, cursor::MoveTo(x, y + i), Print("|"))?;
        execute!(writer, cursor::MoveTo(x + width - 1, y + i), Print("|"))?;
    }
    
    // Bottom border
    execute!(writer, cursor::MoveTo(x, y + height - 1), Print("+"))?;
    for _ in 1..width-1 {
        execute!(writer, Print("-"))?;
    }
    execute!(writer, Print("+"))?;
    
    Ok(())
}

fn create_progress_bar(progress: f64, width: usize, filled_char: char, empty_char: char) -> String {
    let filled_width = (progress * width as f64) as usize;
    let mut bar = String::with_capacity(width);
    
    for i in 0..width {
        if i < filled_width {
            bar.push(filled_char);
        } else {
            bar.push(empty_char);
        }
    }
    bar
}

fn generate_enhanced_tick_animation(state: &Arc<AtomicState>) -> String {
    const ANIMATION_WIDTH: usize = 70;
    const PULSE_SYMBOLS: [char; 4] = ['♪', '♫', '♬', '♭'];
    
    let bpm = state.bpm.load(Ordering::Relaxed);
    let is_running = state.is_running.load(Ordering::Relaxed);
    let tick_count = state.tick_count.load(Ordering::Relaxed);
    
    if !is_running {
        // Use ASCII characters to avoid UTF-8 boundary issues
        let idle_pattern = "=".repeat(ANIMATION_WIDTH);
        return format!("⏸️  {}", idle_pattern);
    }
    
    let elapsed = state.get_last_tick_elapsed();
    let beat_duration = Duration::from_millis(60000 / bpm as u64);
    
    let progress = if beat_duration.as_millis() > 0 {
        (elapsed.as_millis() as f64 / beat_duration.as_millis() as f64).min(1.0)
    } else {
        0.0
    };
    
    let mut animation = vec!['-'; ANIMATION_WIDTH];
    
    // Add measure markers
    let beats_per_measure = 4;
    let marker_spacing = ANIMATION_WIDTH / beats_per_measure;
    for i in 0..beats_per_measure {
        let pos = i * marker_spacing;
        if pos < ANIMATION_WIDTH {
            let measure_num = (tick_count as usize / beats_per_measure) % 4;
            animation[pos] = match measure_num {
                0 => '|', // Strong beat
                1 => ':', // Medium beat
                2 => '|', // Strong beat
                3 => ':', // Medium beat
                _ => '|',
            };
        }
    }
    
    // Moving beat indicator
    let tick_pos = (progress * (ANIMATION_WIDTH - 1) as f64) as usize;
    if tick_pos < ANIMATION_WIDTH {
        let pulse_index = (tick_count as usize) % PULSE_SYMBOLS.len();
        animation[tick_pos] = PULSE_SYMBOLS[pulse_index];
    }
    
    // Beat emphasis effect
    let emphasis_duration = Duration::from_millis(150);
    if elapsed < emphasis_duration {
        let fade_progress = elapsed.as_millis() as f64 / emphasis_duration.as_millis() as f64;
        let intensity = ((1.0 - fade_progress) * 4.0) as usize;
        
        for i in 0..=intensity {
            if tick_pos >= i && tick_pos + i < ANIMATION_WIDTH {
                if i == 0 {
                    animation[tick_pos] = '*';
                } else if i <= 2 {
                    if tick_pos >= i { animation[tick_pos - i] = 'o'; }
                    if tick_pos + i < ANIMATION_WIDTH { animation[tick_pos + i] = 'o'; }
                }
            }
        }
    }
    
    format!("🎼 {}", animation.into_iter().collect::<String>())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AtomicState::new());
    let sound_cache = Arc::new(SoundCache::new());
    let ui_cache = Arc::new(Mutex::new(UICache::new()));
    
    let (tick_tx, tick_rx) = mpsc::channel();
    let (audio_tx, audio_rx) = mpsc::channel::<AudioCommand>();
    
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    
    let state_clone = Arc::clone(&state);
    let sound_cache_clone = Arc::clone(&sound_cache);
    let tick_tx_clone = tick_tx.clone();
    let audio_tx_clone = audio_tx.clone();
    
    thread::spawn(move || {
        metronome_loop(state_clone, sound_cache_clone, tick_tx_clone, audio_tx_clone);
    });
    
    enable_raw_mode()?;
    execute!(io::stdout(), cursor::Hide, Clear(ClearType::All))?;
    
    let stdout = io::stdout();
    let mut buffered_stdout = BufWriter::new(stdout);
    
    let mut last_ui_update = Instant::now();
    const UI_UPDATE_INTERVAL: Duration = Duration::from_millis(16); // 60 FPS
    
    let mut input_check_time = Instant::now();
    const INPUT_CHECK_INTERVAL: Duration = Duration::from_millis(8);
    
    loop {
        let now = Instant::now();
        
        let should_update_ui = now.duration_since(last_ui_update) >= UI_UPDATE_INTERVAL ||
                              state.ui_dirty.load(Ordering::Relaxed);
        
        if should_update_ui {
            display_enhanced_ui(&state, &ui_cache, &mut buffered_stdout)?;
            state.ui_dirty.store(false, Ordering::Relaxed);
            last_ui_update = now;
        }

        if let Ok(cmd) = audio_rx.try_recv() {
            match cmd {
                AudioCommand::PlayTick(sound_data) => {
                    let source = rodio::buffer::SamplesBuffer::new(1, 44100, sound_data);
                    sink.append(source);
                }
                AudioCommand::Stop => break,
            }
        }
        
        if let Ok(_) = tick_rx.try_recv() {
            state.ui_dirty.store(true, Ordering::Relaxed);
        }
        
        if now.duration_since(input_check_time) >= INPUT_CHECK_INTERVAL {
            if poll(Duration::from_millis(0))? {
                match read()? {
                    Event::Key(key_event) => {
                        if key_event.kind == KeyEventKind::Press {
                            let mut needs_ui_update = true;
                            match key_event.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    let _ = audio_tx.send(AudioCommand::Stop);
                                    break;
                                }
                                KeyCode::Char(' ') | KeyCode::Enter => toggle_metronome(&state),
                                KeyCode::Char('r') => toggle_random_mode(&state),
                                KeyCode::Up => adjust_bpm(&state, 5),
                                KeyCode::Down => adjust_bpm(&state, -5),
                                KeyCode::Right => adjust_bpm(&state, 1),
                                KeyCode::Left => adjust_bpm(&state, -1),
                                KeyCode::Char('+') | KeyCode::Char('=') => adjust_random_count(&state, 10),
                                KeyCode::Char('-') | KeyCode::Char('_') => adjust_random_count(&state, -10),
                                KeyCode::Char('s') | KeyCode::Char('n') => cycle_sound(&state, true),
                                KeyCode::Char('a') | KeyCode::Char('p') => cycle_sound(&state, false),
                                KeyCode::Char('t') => {
                                    test_current_sound(&state, &sound_cache, &audio_tx);
                                    needs_ui_update = false;
                                }
                                KeyCode::Char('v') => adjust_volume(&state, 10),
                                KeyCode::Char('c') => adjust_volume(&state, -10),
                                KeyCode::F(1) => set_preset_bpm(&state, 60),   // Slow
                                KeyCode::F(2) => set_preset_bpm(&state, 120),  // Medium
                                KeyCode::F(3) => set_preset_bpm(&state, 180),  // Fast
                                KeyCode::F(4) => set_preset_bpm(&state, 200),  // Very Fast
                                _ => needs_ui_update = false,
                            }
                            
                            if needs_ui_update {
                                state.ui_dirty.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    _ => {}
                }
            }
            input_check_time = now;
        }
        
        thread::sleep(Duration::from_millis(1));
    }
    
    execute!(buffered_stdout, cursor::Show, Clear(ClearType::All))?;
    buffered_stdout.flush()?;
    disable_raw_mode()?;
    
    // Fancy goodbye message
    println!("\n* ======================================= *");
    println!("   Thank you for using CLI Metronome!");
    println!("   Keep the rhythm alive! ♪");
    println!("* ======================================= *\n");
    
    Ok(())
}

fn display_enhanced_ui(
    state: &Arc<AtomicState>, 
    ui_cache: &Arc<Mutex<UICache>>,
    writer: &mut BufWriter<Stdout>
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cache = ui_cache.lock().unwrap();
    
    let current_bpm = state.bpm.load(Ordering::Relaxed);
    let current_sound = state.get_sound_type();
    let current_status = state.is_running.load(Ordering::Relaxed);
    let current_random_mode = state.random_mode.load(Ordering::Relaxed);
    let current_remaining_ticks = state.remaining_ticks.load(Ordering::Relaxed);
    let current_random_count = state.random_count.load(Ordering::Relaxed);
    let current_tick_count = state.tick_count.load(Ordering::Relaxed);
    let current_volume = state.volume.load(Ordering::Relaxed);
    
    if cache.first_render {
        execute!(writer, Clear(ClearType::All))?;
        
        // Animated title with gradient effect
        execute!(
            writer,
            cursor::MoveTo(25, TITLE_ROW),
            SetAttribute(Attribute::Bold),
            SetForegroundColor(Color::Magenta),
            Print("🎵 ═══ "),
            SetForegroundColor(Color::Blue),
            Print("CLI METRONOME STUDIO"),
            SetForegroundColor(Color::Magenta),
            Print(" ═══ 🎵"),
            ResetColor,
        )?;
        
        execute!(
            writer,
            cursor::MoveTo(30, SUBTITLE_ROW),
            SetForegroundColor(Color::DarkGrey),
            Print("♪ Professional Rhythm Training Tool ♪"),
            ResetColor,
        )?;
        
        // Main divider
        execute!(
            writer,
            cursor::MoveTo(10, DIVIDER_ROW),
            SetForegroundColor(Color::Cyan),
            Print("==================================================================="),
            ResetColor,
        )?;
        
        cache.first_render = false;
    }
    
    // Enhanced Beat Animation
    let animation = generate_enhanced_tick_animation(state);
    if animation != cache.animation_buffer {
        cache.animation_buffer = animation.clone();
        
        execute!(writer, cursor::MoveTo(5, ANIMATION_ROW))?;
        draw_box_border(writer, 5, ANIMATION_ROW, 80, 3)?;
        
        execute!(
            writer,
            cursor::MoveTo(7, ANIMATION_ROW + 1),
            Clear(ClearType::UntilNewLine),
        )?;
        
        if current_status {
            execute!(
                writer,
                SetForegroundColor(Color::Green),
                SetAttribute(Attribute::Bold),
                Print(&animation),
                ResetColor,
            )?;
        } else {
            execute!(
                writer,
                SetForegroundColor(Color::DarkGrey),
                Print(&animation),
                ResetColor,
            )?;
        }
    }
    
    // BPM Panel with visual meter
    if current_bpm != cache.last_bpm || cache.first_render {
        execute!(writer, cursor::MoveTo(10, BPM_PANEL_ROW))?;
        draw_box_border(writer, 10, BPM_PANEL_ROW, 30, 4)?;
        
        execute!(
            writer,
            cursor::MoveTo(12, BPM_PANEL_ROW + 1),
            SetForegroundColor(Color::Yellow),
            SetAttribute(Attribute::Bold),
            Print("⚡ BPM: "),
            SetForegroundColor(Color::White),
            Print(&format!("{:3}", current_bpm)),
            ResetColor,
        )?;
        
        // The BPM visual meter
        let bpm_progress = (current_bpm - 30) as f64 / (300 - 30) as f64;
        let meter = create_progress_bar(bpm_progress, 20, '#', '.');
        execute!(
            writer,
            cursor::MoveTo(12, BPM_PANEL_ROW + 2),
            SetForegroundColor(if current_bpm > 150 { Color::Red } else if current_bpm > 100 { Color::Yellow } else { Color::Green }),
            Print(&meter),
            ResetColor,
        )?;
        
        cache.last_bpm = current_bpm;
    }
    
    // Sound Panel
    if current_sound != cache.last_sound || cache.first_render {
        execute!(writer, cursor::MoveTo(45, BPM_PANEL_ROW))?;
        draw_box_border(writer, 45, BPM_PANEL_ROW, 25, 4)?;
        
        execute!(
            writer,
            cursor::MoveTo(47, BPM_PANEL_ROW + 1),
            SetForegroundColor(Color::Magenta),
            SetAttribute(Attribute::Bold),
            Print("🔊 Sound: "),
            ResetColor,
        )?;
        
        execute!(
            writer,
            cursor::MoveTo(47, BPM_PANEL_ROW + 2),
            SetForegroundColor(Color::White),
            Print(&format!("{} {}", current_sound.icon(), current_sound.name())),
            ResetColor,
        )?;
        
        cache.last_sound = current_sound;
    }
    
    // Status Panel with beat counter
    if current_status != cache.last_status || current_tick_count != cache.last_tick_count || cache.first_render {
        execute!(writer, cursor::MoveTo(10, STATUS_PANEL_ROW))?;
        draw_box_border(writer, 10, STATUS_PANEL_ROW, 35, 4)?;
        
        execute!(
            writer,
            cursor::MoveTo(12, STATUS_PANEL_ROW + 1),
            SetAttribute(Attribute::Bold),
        )?;
        
        if current_status {
            let beats_per_measure = (current_tick_count % 4) + 1;
            execute!(
                writer,
                SetForegroundColor(Color::Green),
                Print(&format!("▶️  PLAYING • Beat #{} • {}/4", current_tick_count, beats_per_measure)),
                ResetColor,
            )?;
            
            // Beat dots visualization
            execute!(writer, cursor::MoveTo(12, STATUS_PANEL_ROW + 2))?;
            for i in 1..=4 {
                if i <= beats_per_measure {
                    execute!(writer, SetForegroundColor(Color::Green), Print("* "), ResetColor)?;
                } else {
                    execute!(writer, SetForegroundColor(Color::DarkGrey), Print("o "), ResetColor)?;
                }
            }
        } else {
            execute!(
                writer,
                SetForegroundColor(Color::Red),
                Print("⏹️  STOPPED"),
                ResetColor,
            )?;
        }
        
        cache.last_status = current_status;
        cache.last_tick_count = current_tick_count;
    }
    
    // Random Mode Panel
    if current_random_mode != cache.last_random_mode || current_remaining_ticks != cache.last_remaining_ticks || cache.first_render {
        execute!(writer, cursor::MoveTo(50, STATUS_PANEL_ROW))?;
        draw_box_border(writer, 50, STATUS_PANEL_ROW, 30, 4)?;
        
        execute!(
            writer,
            cursor::MoveTo(52, STATUS_PANEL_ROW + 1),
            SetAttribute(Attribute::Bold),
        )?;
        
        if current_random_mode {
            execute!(
                writer,
                SetForegroundColor(Color::Yellow),
                Print("🎲 RANDOM MODE"),
                ResetColor,
            )?;
            
            execute!(
                writer,
                cursor::MoveTo(52, STATUS_PANEL_ROW + 2),
                SetForegroundColor(Color::White),
                Print(&format!("Next change: {} ticks", current_remaining_ticks)),
                ResetColor,
            )?;
        } else {
            execute!(
                writer,
                SetForegroundColor(Color::DarkGrey),
                Print("🎯 FIXED BPM"),
                ResetColor,
            )?;
        }
        
        cache.last_random_mode = current_random_mode;
        cache.last_remaining_ticks = current_remaining_ticks;
    }
    
    // Volume Panel
    if current_volume != cache.last_volume || cache.first_render {
        execute!(writer, cursor::MoveTo(10, VOLUME_PANEL_ROW))?;
        draw_box_border(writer, 10, VOLUME_PANEL_ROW, 25, 4)?;
        
        execute!(
            writer,
            cursor::MoveTo(12, VOLUME_PANEL_ROW + 1),
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            Print(&format!("🔉 Volume: {}%", current_volume)),
            ResetColor,
        )?;
        
        // Volume bar
        let volume_progress = current_volume as f64 / 100.0;
        let volume_bar = create_progress_bar(volume_progress, 15, '#', '.');
        execute!(
            writer,
            cursor::MoveTo(12, VOLUME_PANEL_ROW + 2),
            SetForegroundColor(Color::Cyan),
            Print(&volume_bar),
            ResetColor,
        )?;
        
        cache.last_volume = current_volume;
    }
    
    // Enhanced Controls Section
    if cache.first_render {
        execute!(
            writer,
            cursor::MoveTo(25, CONTROLS_TITLE_ROW),
            SetForegroundColor(Color::Yellow),
            SetAttribute(Attribute::Bold),
            Print("🎹 ═══ CONTROL PANEL ═══ 🎹"),
            ResetColor,
        )?;
        
        let controls = [
            ("⏯️  SPACE/ENTER", "Start/Stop metronome", Color::Green),
            ("🎲 R", "Toggle random BPM mode", Color::Yellow),
            ("⬆️⬇️ ↑/↓", "Adjust BPM by ±5", Color::Cyan),
            ("⬅️➡️ ←/→", "Adjust BPM by ±1", Color::Cyan),
            ("➕➖ +/-", "Adjust random count ±10", Color::Magenta),
            ("🔊 S/N", "Next sound", Color::Blue),
            ("🔉 A/P", "Previous sound", Color::Blue),
            ("🧪 T", "Test current sound", Color::White),
            ("🔊 V/C", "Volume up/down", Color::Cyan),
            ("⚡ F1-F4", "BPM presets (60/120/180/200)", Color::Red),
            ("❌ Q/ESC", "Quit application", Color::Red),
        ];
        
        for (i, (key, desc, color)) in controls.iter().enumerate() {
            execute!(
                writer,
                cursor::MoveTo(15, CONTROLS_START_ROW + i as u16),
                SetForegroundColor(*color),
                Print(&format!("{:15}", key)),
                SetForegroundColor(Color::White),
                Print(" - "),
                SetForegroundColor(Color::DarkGrey),
                Print(desc),
                ResetColor,
            )?;
        }
        
        // Sound selection grid
        execute!(
            writer,
            cursor::MoveTo(20, SOUNDS_SECTION_ROW),
            SetForegroundColor(Color::Cyan),
            SetAttribute(Attribute::Bold),
            Print("🎵 ═══ AVAILABLE SOUNDS ═══ 🎵"),
            ResetColor,
        )?;
        
        let sounds_display = format!(
            "  {} Beep  {} Kick  {} Click  {} Cowbell  {} Hi-hat  {} Square  {} Triangle  {} Woodblock",
            SoundType::Beep.icon(),
            SoundType::Kick.icon(),
            SoundType::Click.icon(),
            SoundType::Cowbell.icon(),
            SoundType::Hihat.icon(),
            SoundType::Square.icon(),
            SoundType::Triangle.icon(),
            SoundType::Woodblock.icon()
        );
        
        execute!(
            writer,
            cursor::MoveTo(5, SOUNDS_SECTION_ROW + 1),
            SetForegroundColor(Color::White),
            Print(&sounds_display),
            ResetColor,
        )?;
        
        // Footer with tips
        execute!(
            writer,
            cursor::MoveTo(15, FOOTER_ROW),
            SetForegroundColor(Color::DarkGrey),
            Print("💡 Pro tip: Use random mode for practice • F-keys for quick BPM presets • V/C for volume"),
            ResetColor,
        )?;
    }
    
    writer.flush()?;
    Ok(())
}

fn metronome_loop(
    state: Arc<AtomicState>,
    sound_cache: Arc<SoundCache>,
    tick_tx: mpsc::Sender<()>,
    audio_tx: mpsc::Sender<AudioCommand>,
) {
    let mut last_tick = Instant::now();
    let mut current_interval = Duration::from_millis(500);
    let mut rng = rand::thread_rng();
    
    loop {
        let should_tick = {
            if !state.is_running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            
            let bpm = state.bpm.load(Ordering::Relaxed);
            let new_interval = Duration::from_millis(60000 / bpm as u64);
            if new_interval != current_interval {
                current_interval = new_interval;
            }
            
            last_tick.elapsed() >= current_interval
        };
        
        if should_tick {
            state.update_tick();
            
            let sound_type = state.get_sound_type();
            let mut sound_data = sound_cache.get_sound(sound_type).clone();
            
            // Apply volume scaling
            let volume = state.volume.load(Ordering::Relaxed) as f32 / 100.0;
            for sample in &mut sound_data {
                *sample *= volume;
            }
            
            let _ = audio_tx.send(AudioCommand::PlayTick(sound_data));
            last_tick = Instant::now();
            
            if state.random_mode.load(Ordering::Relaxed) {
                let mut remaining = state.remaining_ticks.load(Ordering::Relaxed);
                if remaining == 0 {
                    remaining = state.random_count.load(Ordering::Relaxed);
                }
                
                remaining -= 1;
                state.remaining_ticks.store(remaining, Ordering::Relaxed);
                
                if remaining == 0 {
                    let new_bpm = rng.gen_range(60..=200);
                    state.bpm.store(new_bpm, Ordering::Relaxed);
                    state.remaining_ticks.store(state.random_count.load(Ordering::Relaxed), Ordering::Relaxed);
                }
            }
            
            let _ = tick_tx.send(());
        } else {
            let time_to_next_tick = current_interval.saturating_sub(last_tick.elapsed());
            let sleep_time = time_to_next_tick.min(Duration::from_millis(2));
            thread::sleep(sleep_time);
        }
    }
}

fn toggle_metronome(state: &Arc<AtomicState>) {
    let was_running = state.is_running.load(Ordering::Relaxed);
    state.is_running.store(!was_running, Ordering::Relaxed);
    
    if !was_running && state.random_mode.load(Ordering::Relaxed) && 
       state.remaining_ticks.load(Ordering::Relaxed) == 0 {
        state.remaining_ticks.store(state.random_count.load(Ordering::Relaxed), Ordering::Relaxed);
    }
    
    if !was_running {
        state.tick_count.store(0, Ordering::Relaxed);
    }
    
    state.ui_dirty.store(true, Ordering::Relaxed);
}

fn toggle_random_mode(state: &Arc<AtomicState>) {
    let was_random = state.random_mode.load(Ordering::Relaxed);
    state.random_mode.store(!was_random, Ordering::Relaxed);
    
    if !was_random {
        if state.is_running.load(Ordering::Relaxed) {
            state.remaining_ticks.store(state.random_count.load(Ordering::Relaxed), Ordering::Relaxed);
        }
    } else {
        state.remaining_ticks.store(0, Ordering::Relaxed);
    }
    state.ui_dirty.store(true, Ordering::Relaxed);
}

fn adjust_bpm(state: &Arc<AtomicState>, change: i32) {
    let current = state.bpm.load(Ordering::Relaxed);
    let new_bpm = (current as i32 + change).max(30).min(300) as u32;
    state.bpm.store(new_bpm, Ordering::Relaxed);
    state.ui_dirty.store(true, Ordering::Relaxed);
}

fn adjust_random_count(state: &Arc<AtomicState>, change: i32) {
    let current = state.random_count.load(Ordering::Relaxed);
    let new_count = (current as i32 + change).max(10).min(1000) as u32;
    state.random_count.store(new_count, Ordering::Relaxed);
    
    if state.random_mode.load(Ordering::Relaxed) && state.is_running.load(Ordering::Relaxed) {
        let remaining = state.remaining_ticks.load(Ordering::Relaxed);
        if new_count > remaining {
            state.remaining_ticks.store(new_count, Ordering::Relaxed);
        }
    }
    state.ui_dirty.store(true, Ordering::Relaxed);
}

fn adjust_volume(state: &Arc<AtomicState>, change: i32) {
    let current = state.volume.load(Ordering::Relaxed);
    let new_volume = (current as i32 + change).max(0).min(100) as u32;
    state.volume.store(new_volume, Ordering::Relaxed);
    state.ui_dirty.store(true, Ordering::Relaxed);
}

fn set_preset_bpm(state: &Arc<AtomicState>, bpm: u32) {
    state.bpm.store(bpm, Ordering::Relaxed);
    state.ui_dirty.store(true, Ordering::Relaxed);
}

fn cycle_sound(state: &Arc<AtomicState>, forward: bool) {
    let current = state.get_sound_type();
    let new_sound = if forward { current.next() } else { current.prev() };
    state.set_sound_type(new_sound);
    state.ui_dirty.store(true, Ordering::Relaxed);
}

fn test_current_sound(
    state: &Arc<AtomicState>, 
    sound_cache: &Arc<SoundCache>,
    audio_tx: &mpsc::Sender<AudioCommand>
) {
    let sound_type = state.get_sound_type();
    let mut sound_data = sound_cache.get_sound(sound_type).clone();
    
    // Apply volume scaling
    let volume = state.volume.load(Ordering::Relaxed) as f32 / 100.0;
    for sample in &mut sound_data {
        *sample *= volume;
    }
    
    let _ = audio_tx.send(AudioCommand::PlayTick(sound_data));
}