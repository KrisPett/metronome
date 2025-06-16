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
    style::{Color, Print, ResetColor, SetForegroundColor},
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
    last_tick_time: AtomicU64, // Store as nanoseconds since epoch
    tick_count: AtomicU32,
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
            return Duration::from_secs(999); // Return a large duration if no tick yet
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
    first_render: bool,
    bpm_buffer: String,
    sound_buffer: String,
    status_buffer: String,
    ticks_buffer: String,
    count_buffer: String,
    animation_buffer: String,
}

impl UICache {
    fn new() -> Self {
        Self {
            first_render: true,
            bpm_buffer: String::with_capacity(8),
            sound_buffer: String::with_capacity(16),
            status_buffer: String::with_capacity(16),
            ticks_buffer: String::with_capacity(32),
            count_buffer: String::with_capacity(8),
            animation_buffer: String::with_capacity(80),
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

const HEADER_ROW: u16 = 0;
const ANIMATION_ROW: u16 = 1;
const BPM_ROW: u16 = 3;
const SOUND_ROW: u16 = 4;
const STATUS_ROW: u16 = 5;
const RANDOM_MODE_ROW: u16 = 6;
const REMAINING_TICKS_ROW: u16 = 7;
const RANDOM_COUNT_ROW: u16 = 8;
const CONTROLS_START_ROW: u16 = 10;
const SOUNDS_LIST_ROW: u16 = 22;
const TIP_ROW: u16 = 24;

fn generate_tick_animation(state: &Arc<AtomicState>) -> String {
    const ANIMATION_WIDTH: usize = 60;
    const TICK_SYMBOL: char = '♪';
    const BAR_SYMBOL: char = '─';
    
    let bpm = state.bpm.load(Ordering::Relaxed);
    let is_running = state.is_running.load(Ordering::Relaxed);
    let tick_count = state.tick_count.load(Ordering::Relaxed);
    
    if !is_running {
        return format!("{:─<width$}", "", width = ANIMATION_WIDTH);
    }
    
    let elapsed = state.get_last_tick_elapsed();
    let beat_duration = Duration::from_millis(60000 / bpm as u64);
    
    // Calculate progress through current beat (0.0 to 1.0)
    let progress = if beat_duration.as_millis() > 0 {
        (elapsed.as_millis() as f64 / beat_duration.as_millis() as f64).min(1.0)
    } else {
        0.0
    };
    
    // Calculate position of the tick indicator
    let tick_pos = (progress * (ANIMATION_WIDTH - 1) as f64) as usize;
    
    // Create the animation bar
    let mut animation = vec![BAR_SYMBOL; ANIMATION_WIDTH];
    
    // Add tick markers at regular intervals (every 4 beats)
    let beats_per_measure = 4;
    let marker_spacing = ANIMATION_WIDTH / beats_per_measure;
    for i in 0..beats_per_measure {
        let pos = i * marker_spacing;
        if pos < ANIMATION_WIDTH {
            animation[pos] = if (tick_count as usize / beats_per_measure) % 2 == 0 { '|' } else { '┃' };
        }
    }
    
    // Add the moving tick indicator
    if tick_pos < ANIMATION_WIDTH {
        animation[tick_pos] = TICK_SYMBOL;
    }
    
    // Add visual emphasis for recent ticks (fade effect)
    let fade_duration = Duration::from_millis(200);
    if elapsed < fade_duration {
        let fade_progress = elapsed.as_millis() as f64 / fade_duration.as_millis() as f64;
        let emphasis_size = ((1.0 - fade_progress) * 3.0) as usize;
        
        for i in 0..=emphasis_size {
            if tick_pos >= i && tick_pos + i < ANIMATION_WIDTH {
                if i == 0 {
                    animation[tick_pos] = '♫';
                } else {
                    animation[tick_pos - i] = '◦';
                    if tick_pos + i < ANIMATION_WIDTH {
                        animation[tick_pos + i] = '◦';
                    }
                }
            }
        }
    }
    
    animation.into_iter().collect()
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
    execute!(io::stdout(), cursor::Hide)?;
    
    let stdout = io::stdout();
    let mut buffered_stdout = BufWriter::new(stdout);
    
    let mut last_ui_update = Instant::now();
    const UI_UPDATE_INTERVAL: Duration = Duration::from_millis(16); // 60 FPS for smooth animation
    
    let mut input_check_time = Instant::now();
    const INPUT_CHECK_INTERVAL: Duration = Duration::from_millis(8);
    
    loop {
        let now = Instant::now();
        
        // Update UI more frequently for smooth animation
        let should_update_ui = now.duration_since(last_ui_update) >= UI_UPDATE_INTERVAL ||
                              state.ui_dirty.load(Ordering::Relaxed);
        
        if should_update_ui {
            display_ui_optimized(&state, &ui_cache, &mut buffered_stdout)?;
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
                                KeyCode::Char('q') => {
                                    let _ = audio_tx.send(AudioCommand::Stop);
                                    break;
                                }
                                KeyCode::Char(' ') => toggle_metronome(&state),
                                KeyCode::Char('r') => toggle_random_mode(&state),
                                KeyCode::Up => adjust_bpm(&state, 5),
                                KeyCode::Down => adjust_bpm(&state, -5),
                                KeyCode::Right => adjust_bpm(&state, 1),
                                KeyCode::Left => adjust_bpm(&state, -1),
                                KeyCode::Char('+') => adjust_random_count(&state, 10),
                                KeyCode::Char('-') => adjust_random_count(&state, -10),
                                KeyCode::Char('s') => cycle_sound(&state, true),
                                KeyCode::Char('a') => cycle_sound(&state, false),
                                KeyCode::Char('t') => {
                                    test_current_sound(&state, &sound_cache, &audio_tx);
                                    needs_ui_update = false;
                                }
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
    
    execute!(buffered_stdout, cursor::Show)?;
    buffered_stdout.flush()?;
    disable_raw_mode()?;
    println!("\nMetronome stopped. Goodbye!");
    Ok(())
}

fn display_ui_optimized(
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
    
    if cache.first_render {
        execute!(
            writer,
            Clear(ClearType::All),
            cursor::MoveTo(0, HEADER_ROW),
            SetForegroundColor(Color::Blue),
            Print("🎵 CLI METRONOME 🎵"),
            ResetColor,
        )?;
        
        execute!(writer, cursor::MoveTo(0, BPM_ROW), Print("BPM: "))?;
        execute!(writer, cursor::MoveTo(0, SOUND_ROW), Print("Sound: "))?;
        execute!(writer, cursor::MoveTo(0, STATUS_ROW), Print("Status: "))?;
        execute!(writer, cursor::MoveTo(0, RANDOM_MODE_ROW), Print("Random mode: "))?;
        execute!(writer, cursor::MoveTo(0, REMAINING_TICKS_ROW), Print("Remaining ticks: "))?;
        execute!(writer, cursor::MoveTo(0, RANDOM_COUNT_ROW), Print("Random count: "))?;
        
        execute!(
            writer,
            cursor::MoveTo(0, CONTROLS_START_ROW),
            SetForegroundColor(Color::Yellow),
            Print("📋 CONTROLS:"),
            ResetColor,
        )?;
        
        let controls = [
            "  SPACE     - Start/Stop metronome",
            "  R         - Toggle random mode",
            "  ↑/↓       - Adjust BPM by 5",
            "  ←/→       - Adjust BPM by 1",
            "  +/-       - Adjust random count by 10",
            "  S         - Next sound",
            "  A         - Previous sound",
            "  T         - Test current sound",
            "  Q         - Quit",
        ];
        
        for (i, control) in controls.iter().enumerate() {
            execute!(
                writer,
                cursor::MoveTo(0, CONTROLS_START_ROW + 1 + i as u16),
                Print(control),
            )?;
        }
        
        execute!(
            writer,
            cursor::MoveTo(0, SOUNDS_LIST_ROW),
            SetForegroundColor(Color::Cyan),
            Print("🔊 Available sounds:"),
            ResetColor,
        )?;
        execute!(
            writer,
            cursor::MoveTo(0, SOUNDS_LIST_ROW + 1),
            Print("  Beep • Kick • Click • Cowbell • Hi-hat • Square • Triangle • Woodblock"),
        )?;
        
        cache.first_render = false;
    }
    
    // Update tick animation (always update for smooth animation)
    let animation = generate_tick_animation(state);
    if animation != cache.animation_buffer || current_status != cache.last_status {
        cache.animation_buffer = animation.clone();
        
        execute!(
            writer,
            cursor::MoveTo(0, ANIMATION_ROW),
            Clear(ClearType::UntilNewLine),
        )?;
        
        if current_status {
            execute!(
                writer,
                SetForegroundColor(Color::Green),
                Print(&format!("🎼 {}", animation)),
                ResetColor,
            )?;
        } else {
            execute!(
                writer,
                SetForegroundColor(Color::DarkGrey),
                Print(&format!("⏸  {}", animation)),
                ResetColor,
            )?;
        }
    }
    
    if current_bpm != cache.last_bpm {
        cache.bpm_buffer.clear();
        cache.bpm_buffer.push_str(&current_bpm.to_string());
        
        execute!(
            writer,
            cursor::MoveTo(5, BPM_ROW),
            Clear(ClearType::UntilNewLine),
            SetForegroundColor(Color::Cyan),
            Print(&cache.bpm_buffer),
            ResetColor,
        )?;
        cache.last_bpm = current_bpm;
    }
    
    if current_sound != cache.last_sound {
        cache.sound_buffer.clear();
        cache.sound_buffer.push_str(current_sound.name());
        
        execute!(
            writer,
            cursor::MoveTo(7, SOUND_ROW),
            Clear(ClearType::UntilNewLine),
            SetForegroundColor(Color::Magenta),
            Print(&cache.sound_buffer),
            ResetColor,
        )?;
        cache.last_sound = current_sound;
    }
    
    if current_status != cache.last_status {
        cache.status_buffer.clear();
        let (status_text, status_color) = if current_status {
            (format!("RUNNING ♪ (Beat #{})", current_tick_count), Color::Green)
        } else {
            (String::from("STOPPED"), Color::Red)
        };
        cache.status_buffer.push_str(&status_text);
        
        execute!(
            writer,
            cursor::MoveTo(8, STATUS_ROW),
            Clear(ClearType::UntilNewLine),
            SetForegroundColor(status_color),
            Print(&cache.status_buffer),
            ResetColor,
        )?;
        cache.last_status = current_status;
    } else if current_status && current_tick_count != cache.last_tick_count {
        // Update beat counter when running
        cache.status_buffer.clear();
        cache.status_buffer.push_str(&format!("RUNNING ♪ (Beat #{})", current_tick_count));
        
        execute!(
            writer,
            cursor::MoveTo(8, STATUS_ROW),
            Clear(ClearType::UntilNewLine),
            SetForegroundColor(Color::Green),
            Print(&cache.status_buffer),
            ResetColor,
        )?;
        cache.last_tick_count = current_tick_count;
    }
    
    if current_random_mode != cache.last_random_mode {
        execute!(
            writer,
            cursor::MoveTo(13, RANDOM_MODE_ROW),
            Clear(ClearType::UntilNewLine),
        )?;
        
        if current_random_mode {
            execute!(
                writer,
                SetForegroundColor(Color::Yellow),
                Print("🎲 ACTIVE"),
                ResetColor,
            )?;
        } else {
            execute!(
                writer,
                SetForegroundColor(Color::DarkGrey),
                Print("OFF"),
                ResetColor,
            )?;
        }
        cache.last_random_mode = current_random_mode;
    }
    
    if current_remaining_ticks != cache.last_remaining_ticks {
        cache.ticks_buffer.clear();
        
        execute!(
            writer,
            cursor::MoveTo(17, REMAINING_TICKS_ROW),
            Clear(ClearType::UntilNewLine),
        )?;
        
        if current_random_mode && current_status {
            cache.ticks_buffer.push_str(&current_remaining_ticks.to_string());
            execute!(
                writer,
                SetForegroundColor(Color::White),
                Print(&cache.ticks_buffer),
                ResetColor,
            )?;
        } else if current_random_mode && !current_status {
            execute!(
                writer,
                SetForegroundColor(Color::DarkGrey),
                Print("(Start to begin countdown)"),
                ResetColor,
            )?;
        } else {
            execute!(
                writer,
                SetForegroundColor(Color::DarkGrey),
                Print("-"),
                ResetColor,
            )?;
        }
        cache.last_remaining_ticks = current_remaining_ticks;
    }
    
    if current_random_count != cache.last_random_count {
        cache.count_buffer.clear();
        cache.count_buffer.push_str(&current_random_count.to_string());
        
        execute!(
            writer,
            cursor::MoveTo(15, RANDOM_COUNT_ROW),
            Clear(ClearType::UntilNewLine),
            Print(&cache.count_buffer),
        )?;
        cache.last_random_count = current_random_count;
    }
    
    cache.ticks_buffer.clear();
    cache.ticks_buffer.push_str(&format!("💡 Random mode changes BPM every {} ticks • Animation shows beat progress", current_random_count));
    
    execute!(
        writer,
        cursor::MoveTo(0, TIP_ROW),
        Clear(ClearType::UntilNewLine),
        SetForegroundColor(Color::DarkGrey),
        Print(&cache.ticks_buffer),
        ResetColor,
    )?;
    
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
            // Update tick timing for animation
            state.update_tick();
            
            let sound_type = state.get_sound_type();
            let sound_data = sound_cache.get_sound(sound_type).clone();
            
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
    
    // Reset tick counter when starting
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
    let sound_data = sound_cache.get_sound(sound_type).clone();
    let _ = audio_tx.send(AudioCommand::PlayTick(sound_data));
}