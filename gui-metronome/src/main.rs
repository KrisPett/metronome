use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc;
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

// Track what was last displayed to detect changes
#[derive(Clone, Debug)]
struct UIState {
    last_bpm: u32,
    last_sound: SoundType,
    last_status: bool,
    last_random_mode: bool,
    last_remaining_ticks: u32,
    last_random_count: u32,
    first_render: bool,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            last_bpm: 0,
            last_sound: SoundType::Beep,
            last_status: false,
            last_random_mode: false,
            last_remaining_ticks: 0,
            last_random_count: 0,
            first_render: true,
        }
    }
}

#[derive(Clone)]
struct MetronomeState {
    bpm: u32,
    is_running: bool,
    random_mode: bool,
    random_count: u32,
    remaining_ticks: u32,
    sound_type: SoundType,
    ui_dirty: bool,
    ui_state: UIState,
}

impl MetronomeState {
    fn new() -> Self {
        Self {
            bpm: 120,
            is_running: false,
            random_mode: false,
            random_count: 100,
            remaining_ticks: 0,
            sound_type: SoundType::Kick,
            ui_dirty: true,
            ui_state: UIState::default(),
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

// UI Layout constants - define where each element appears
const HEADER_ROW: u16 = 0;
const BPM_ROW: u16 = 2;
const SOUND_ROW: u16 = 3;
const STATUS_ROW: u16 = 4;
const RANDOM_MODE_ROW: u16 = 5;
const REMAINING_TICKS_ROW: u16 = 6;
const RANDOM_COUNT_ROW: u16 = 7;
const CONTROLS_START_ROW: u16 = 9;
const SOUNDS_LIST_ROW: u16 = 21;
const TIP_ROW: u16 = 23;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(Mutex::new(MetronomeState::new()));
    let sound_cache = Arc::new(SoundCache::new());
    
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
    
    // Hide cursor for cleaner appearance
    execute!(io::stdout(), cursor::Hide)?;
    
    let mut last_ui_update = Instant::now();
    const UI_UPDATE_INTERVAL: Duration = Duration::from_millis(16); // 60 FPS
    
    loop {
        let should_update_ui = {
            let state_guard = state.lock().unwrap();
            state_guard.ui_dirty && last_ui_update.elapsed() >= UI_UPDATE_INTERVAL
        };
        
        if should_update_ui {
            display_ui_dynamic(&state)?;
            {
                let mut state_guard = state.lock().unwrap();
                state_guard.ui_dirty = false;
            }
            last_ui_update = Instant::now();
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
            let mut state_guard = state.lock().unwrap();
            state_guard.ui_dirty = true;
        }
        
        if poll(Duration::from_millis(1))? {
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
                            let mut state_guard = state.lock().unwrap();
                            state_guard.ui_dirty = true;
                        }
                    }
                }
                _ => {}
            }
        }
        
        thread::sleep(Duration::from_millis(1));
    }
    
    // Restore cursor and clean up
    execute!(io::stdout(), cursor::Show)?;
    disable_raw_mode()?;
    println!("\nMetronome stopped. Goodbye!");
    Ok(())
}

fn display_ui_dynamic(state: &Arc<Mutex<MetronomeState>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut state_guard = state.lock().unwrap();
    
    // First time - draw the complete static layout
    if state_guard.ui_state.first_render {
        execute!(io::stdout(), Clear(ClearType::All))?;
        
        // Header
        execute!(
            io::stdout(),
            cursor::MoveTo(0, HEADER_ROW),
            SetForegroundColor(Color::Blue),
            Print("🎵 CLI METRONOME 🎵"),
            ResetColor,
        )?;
        
        // Static labels
        execute!(io::stdout(), cursor::MoveTo(0, BPM_ROW), Print("BPM: "))?;
        execute!(io::stdout(), cursor::MoveTo(0, SOUND_ROW), Print("Sound: "))?;
        execute!(io::stdout(), cursor::MoveTo(0, STATUS_ROW), Print("Status: "))?;
        execute!(io::stdout(), cursor::MoveTo(0, RANDOM_MODE_ROW), Print("Random mode: "))?;
        execute!(io::stdout(), cursor::MoveTo(0, REMAINING_TICKS_ROW), Print("Remaining ticks: "))?;
        execute!(io::stdout(), cursor::MoveTo(0, RANDOM_COUNT_ROW), Print("Random count: "))?;
        
        // Controls section
        execute!(
            io::stdout(),
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
                io::stdout(),
                cursor::MoveTo(0, CONTROLS_START_ROW + 1 + i as u16),
                Print(control),
            )?;
        }
        
        // Sounds list
        execute!(
            io::stdout(),
            cursor::MoveTo(0, SOUNDS_LIST_ROW),
            SetForegroundColor(Color::Cyan),
            Print("🔊 Available sounds:"),
            ResetColor,
        )?;
        execute!(
            io::stdout(),
            cursor::MoveTo(0, SOUNDS_LIST_ROW + 1),
            Print("  Beep • Kick • Click • Cowbell • Hi-hat • Square • Triangle • Woodblock"),
        )?;
        
        state_guard.ui_state.first_render = false;
    }
    
    // Update BPM if changed
    if state_guard.bpm != state_guard.ui_state.last_bpm {
        execute!(
            io::stdout(),
            cursor::MoveTo(5, BPM_ROW),
            Clear(ClearType::UntilNewLine),
            SetForegroundColor(Color::Cyan),
            Print(format!("{}", state_guard.bpm)),
            ResetColor,
        )?;
        state_guard.ui_state.last_bpm = state_guard.bpm;
    }
    
    // Update sound if changed
    if state_guard.sound_type != state_guard.ui_state.last_sound {
        execute!(
            io::stdout(),
            cursor::MoveTo(7, SOUND_ROW),
            Clear(ClearType::UntilNewLine),
            SetForegroundColor(Color::Magenta),
            Print(format!("{}", state_guard.sound_type.name())),
            ResetColor,
        )?;
        state_guard.ui_state.last_sound = state_guard.sound_type;
    }
    
    // Update status if changed
    if state_guard.is_running != state_guard.ui_state.last_status {
        let (status_text, status_color) = if state_guard.is_running {
            ("RUNNING ♪", Color::Green)
        } else {
            ("STOPPED", Color::Red)
        };
        
        execute!(
            io::stdout(),
            cursor::MoveTo(8, STATUS_ROW),
            Clear(ClearType::UntilNewLine),
            SetForegroundColor(status_color),
            Print(status_text),
            ResetColor,
        )?;
        state_guard.ui_state.last_status = state_guard.is_running;
    }
    
    // Update random mode if changed
    if state_guard.random_mode != state_guard.ui_state.last_random_mode {
        execute!(
            io::stdout(),
            cursor::MoveTo(13, RANDOM_MODE_ROW),
            Clear(ClearType::UntilNewLine),
        )?;
        
        if state_guard.random_mode {
            execute!(
                io::stdout(),
                SetForegroundColor(Color::Yellow),
                Print("🎲 ACTIVE"),
                ResetColor,
            )?;
        } else {
            execute!(
                io::stdout(),
                SetForegroundColor(Color::DarkGrey),
                Print("OFF"),
                ResetColor,
            )?;
        }
        state_guard.ui_state.last_random_mode = state_guard.random_mode;
    }
    
    // Update remaining ticks if changed (only show when random mode is active)
    if state_guard.remaining_ticks != state_guard.ui_state.last_remaining_ticks {
        execute!(
            io::stdout(),
            cursor::MoveTo(17, REMAINING_TICKS_ROW),
            Clear(ClearType::UntilNewLine),
        )?;
        
        if state_guard.random_mode && state_guard.is_running {
            execute!(
                io::stdout(),
                SetForegroundColor(Color::White),
                Print(format!("{}", state_guard.remaining_ticks)),
                ResetColor,
            )?;
        } else if state_guard.random_mode && !state_guard.is_running {
            execute!(
                io::stdout(),
                SetForegroundColor(Color::DarkGrey),
                Print("(Start to begin countdown)"),
                ResetColor,
            )?;
        } else {
            execute!(
                io::stdout(),
                SetForegroundColor(Color::DarkGrey),
                Print("-"),
                ResetColor,
            )?;
        }
        state_guard.ui_state.last_remaining_ticks = state_guard.remaining_ticks;
    }
    
    // Update random count if changed
    if state_guard.random_count != state_guard.ui_state.last_random_count {
        execute!(
            io::stdout(),
            cursor::MoveTo(15, RANDOM_COUNT_ROW),
            Clear(ClearType::UntilNewLine),
            Print(format!("{}", state_guard.random_count)),
        )?;
        state_guard.ui_state.last_random_count = state_guard.random_count;
    }
    
    // Update tip text with current random count
    execute!(
        io::stdout(),
        cursor::MoveTo(0, TIP_ROW),
        Clear(ClearType::UntilNewLine),
        SetForegroundColor(Color::DarkGrey),
        Print(format!("💡 Random mode changes BPM every {} ticks", state_guard.random_count)),
        ResetColor,
    )?;
    
    io::stdout().flush()?;
    Ok(())
}

fn metronome_loop(
    state: Arc<Mutex<MetronomeState>>,
    sound_cache: Arc<SoundCache>,
    tick_tx: mpsc::Sender<()>,
    audio_tx: mpsc::Sender<AudioCommand>,
) {
    let mut last_tick = Instant::now();
    let mut current_interval = Duration::from_millis(500);
    let mut rng = rand::thread_rng();
    
    loop {
        let should_tick = {
            let state_guard = state.lock().unwrap();
            if !state_guard.is_running {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            
            let new_interval = Duration::from_millis(60000 / state_guard.bpm as u64);
            if new_interval != current_interval {
                current_interval = new_interval;
            }
            
            last_tick.elapsed() >= current_interval
        };
        
        if should_tick {
            let sound_data = {
                let state_guard = state.lock().unwrap();
                sound_cache.get_sound(state_guard.sound_type).clone()
            };
            
            let _ = audio_tx.send(AudioCommand::PlayTick(sound_data));
            last_tick = Instant::now();
            
            {
                let mut state_guard = state.lock().unwrap();
                if state_guard.random_mode {
                    if state_guard.remaining_ticks == 0 {
                        state_guard.remaining_ticks = state_guard.random_count;
                    }
                    
                    state_guard.remaining_ticks -= 1;
                    
                    if state_guard.remaining_ticks == 0 {
                        state_guard.bpm = rng.gen_range(60..=200);
                        state_guard.remaining_ticks = state_guard.random_count;
                    }
                }
            }
            
            let _ = tick_tx.send(());
        } else {
            let time_to_next_tick = current_interval.saturating_sub(last_tick.elapsed());
            let sleep_time = time_to_next_tick.min(Duration::from_millis(5));
            thread::sleep(sleep_time);
        }
    }
}

fn toggle_metronome(state: &Arc<Mutex<MetronomeState>>) {
    let mut state_guard = state.lock().unwrap();
    state_guard.is_running = !state_guard.is_running;
    
    if state_guard.is_running && state_guard.random_mode && state_guard.remaining_ticks == 0 {
        state_guard.remaining_ticks = state_guard.random_count;
    }
}

fn toggle_random_mode(state: &Arc<Mutex<MetronomeState>>) {
    let mut state_guard = state.lock().unwrap();
    state_guard.random_mode = !state_guard.random_mode;
    
    if state_guard.random_mode {
        if state_guard.is_running {
            state_guard.remaining_ticks = state_guard.random_count;
        }
    } else {
        state_guard.remaining_ticks = 0;
    }
}

fn adjust_bpm(state: &Arc<Mutex<MetronomeState>>, change: i32) {
    let mut state_guard = state.lock().unwrap();
    let new_bpm = (state_guard.bpm as i32 + change).max(30).min(300) as u32;
    state_guard.bpm = new_bpm;
}

fn adjust_random_count(state: &Arc<Mutex<MetronomeState>>, change: i32) {
    let mut state_guard = state.lock().unwrap();
    let new_count = (state_guard.random_count as i32 + change).max(10).min(1000) as u32;
    state_guard.random_count = new_count;
    
    if state_guard.random_mode && state_guard.is_running {
        if new_count > state_guard.remaining_ticks {
            state_guard.remaining_ticks = new_count;
        }
    }
}

fn cycle_sound(state: &Arc<Mutex<MetronomeState>>, forward: bool) {
    let mut state_guard = state.lock().unwrap();
    state_guard.sound_type = if forward {
        state_guard.sound_type.next()
    } else {
        state_guard.sound_type.prev()
    };
}

fn test_current_sound(
    state: &Arc<Mutex<MetronomeState>>, 
    sound_cache: &Arc<SoundCache>,
    audio_tx: &mpsc::Sender<AudioCommand>
) {
    let sound_data = {
        let state_guard = state.lock().unwrap();
        sound_cache.get_sound(state_guard.sound_type).clone()
    };
    let _ = audio_tx.send(AudioCommand::PlayTick(sound_data));
}