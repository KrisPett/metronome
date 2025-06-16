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

use crate::utlitites::sound::{create_beep_sound, create_kick_sound, create_click_sound, create_cowbell_sound, create_hihat_sound, create_square_sound, create_triangle_sound, create_wood_block_sound};
mod utlitites;

#[derive(Clone, Copy, Debug)]
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
    fn next(&self) -> Self {
        match self {
            SoundType::Beep => SoundType::Kick,
            SoundType::Kick => SoundType::Click,
            SoundType::Click => SoundType::Cowbell,
            SoundType::Cowbell => SoundType::Hihat,
            SoundType::Hihat => SoundType::Square,
            SoundType::Square => SoundType::Triangle,
            SoundType::Triangle => SoundType::Woodblock,
            SoundType::Woodblock => SoundType::Beep,
        }
    }

    fn prev(&self) -> Self {
        match self {
            SoundType::Beep => SoundType::Woodblock,
            SoundType::Kick => SoundType::Beep,
            SoundType::Click => SoundType::Kick,
            SoundType::Cowbell => SoundType::Click,
            SoundType::Hihat => SoundType::Cowbell,
            SoundType::Square => SoundType::Hihat,
            SoundType::Triangle => SoundType::Square,
            SoundType::Woodblock => SoundType::Triangle,
        }
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

#[derive(Clone)]
struct MetronomeState {
    bpm: u32,
    is_running: bool,
    random_mode: bool,
    random_count: u32,
    remaining_ticks: u32,
    sound_type: SoundType,
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
        }
    }
}

enum AudioCommand {
    PlayTick(Vec<f32>),
    Stop,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(Mutex::new(MetronomeState::new()));
    
    let (tick_tx, tick_rx) = mpsc::channel();
    let (audio_tx, audio_rx) = mpsc::channel::<AudioCommand>();
    
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    
    let state_clone = Arc::clone(&state);
    let tick_tx_clone = tick_tx.clone();
    let audio_tx_clone = audio_tx.clone();
    
    thread::spawn(move || {
        metronome_loop(state_clone, tick_tx_clone, audio_tx_clone);
    });
    
    enable_raw_mode()?;
    
    loop {
        display_ui(&state)?;
        
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
            // Tick received, UI will be updated in next iteration
        }
        
        if poll(Duration::from_millis(50))? {
            match read()? {
                Event::Key(key_event) => {
                    if key_event.kind == crossterm::event::KeyEventKind::Press {
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
                            KeyCode::Char('t') => test_current_sound(&state, &audio_tx),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
    
    disable_raw_mode()?;
    println!("\nMetronome stopped. Goodbye!");
    Ok(())
}

fn metronome_loop(
    state: Arc<Mutex<MetronomeState>>,
    tick_tx: mpsc::Sender<()>,
    audio_tx: mpsc::Sender<AudioCommand>,
) {
    let mut last_tick = Instant::now();
    
    loop {
        let (should_tick, interval, sound_data) = {
            let state_guard = state.lock().unwrap();
            if !state_guard.is_running {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            
            let interval = Duration::from_millis(60000 / state_guard.bpm as u64);
            let sound_data = state_guard.sound_type.create_sound();
            (last_tick.elapsed() >= interval, interval, sound_data)
        };
        
        if should_tick {
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
                        let mut rng = rand::thread_rng();
                        state_guard.bpm = rng.gen_range(60..=200);
                        state_guard.remaining_ticks = state_guard.random_count;
                    }
                }
            }
            
            let _ = tick_tx.send(());
        }
        
        thread::sleep(Duration::from_millis(1));
    }
}

fn display_ui(state: &Arc<Mutex<MetronomeState>>) -> Result<(), Box<dyn std::error::Error>> {
    let state_guard = state.lock().unwrap();
    
    execute!(
        io::stdout(),
        Clear(ClearType::All),
        cursor::MoveTo(0, 0),
    )?;
    
    println!("🎵 CLI METRONOME 🎵\n");
    
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Cyan),
        Print(format!("BPM: {}\n", state_guard.bpm)),
        ResetColor,
    )?;
    
    execute!(
        io::stdout(),
        Print("Sound: "),
        SetForegroundColor(Color::Magenta),
        Print(format!("{}\n", state_guard.sound_type.name())),
        ResetColor,
    )?;
    
    let status = if state_guard.is_running { "RUNNING" } else { "STOPPED" };
    let status_color = if state_guard.is_running { Color::Green } else { Color::Red };
    
    execute!(
        io::stdout(),
        Print("Status: "),
        SetForegroundColor(status_color),
        Print(format!("{}\n", status)),
        ResetColor,
    )?;
    
    if state_guard.random_mode {
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Yellow),
            Print("🎲 RANDOM MODE ACTIVE\n"),
            ResetColor,
        )?;
        
        if state_guard.is_running {
            execute!(
                io::stdout(),
                Print(format!("Remaining ticks: {}\n", state_guard.remaining_ticks)),
            )?;
        } else {
            execute!(
                io::stdout(),
                SetForegroundColor(Color::DarkGrey),
                Print("(Start metronome to begin countdown)\n"),
                ResetColor,
            )?;
        }
        
        println!("Random count setting: {}", state_guard.random_count);
    } else {
        execute!(
            io::stdout(),
            SetForegroundColor(Color::DarkGrey),
            Print("Random mode: OFF\n"),
            ResetColor,
        )?;
    }
    
    println!("\n📋 CONTROLS:");
    println!("  SPACE     - Start/Stop metronome");
    println!("  R         - Toggle random mode");
    println!("  ↑/↓       - Adjust BPM by 5");
    println!("  ←/→       - Adjust BPM by 1");
    println!("  +/-       - Adjust random count by 10");
    println!("  S         - Next sound");
    println!("  A         - Previous sound");
    println!("  T         - Test current sound");
    println!("  Q         - Quit");
    
    println!("\n🔊 Available sounds:");
    println!("  Beep • Kick • Click • Cowbell • Hi-hat • Square • Triangle • Woodblock");
    
    println!("\n💡 Random mode will change BPM every {} ticks", state_guard.random_count);
    
    io::stdout().flush()?;
    
    Ok(())
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

fn test_current_sound(state: &Arc<Mutex<MetronomeState>>, audio_tx: &mpsc::Sender<AudioCommand>) {
    let sound_data = {
        let state_guard = state.lock().unwrap();
        state_guard.sound_type.create_sound()
    };
    let _ = audio_tx.send(AudioCommand::PlayTick(sound_data));
}
