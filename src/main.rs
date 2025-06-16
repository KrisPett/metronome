use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc;
use rodio::{OutputStream, Sink, Source};
use crossterm::{
    cursor,
    event::{poll, read, Event, KeyCode, KeyEventKind},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use rand::Rng;

#[derive(Clone)]
struct MetronomeState {
    bpm: u32,
    is_running: bool,
    random_mode: bool,
    random_count: u32,
    remaining_ticks: u32,
}

impl MetronomeState {
    fn new() -> Self {
        Self {
            bpm: 120,
            is_running: false,
            random_mode: false,
            random_count: 100,
            remaining_ticks: 0,
        }
    }
}

enum AudioCommand {
    PlayTick,
    Stop,
}

fn create_beep_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let frequency = 800.0;
    let duration_ms = 50;
    let samples = (sample_rate * duration_ms / 1000) as usize;
    
    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * frequency * 2.0 * std::f32::consts::PI).sin() * 0.3;
        wave.push(sample);
    }
    wave
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(Mutex::new(MetronomeState::new()));
    
    // Channels for communication between threads
    let (tick_tx, tick_rx) = mpsc::channel(); // For tick notifications
    let (audio_tx, audio_rx) = mpsc::channel::<AudioCommand>(); // For audio commands
    
    // Initialize audio in main thread
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    let beep_sound = create_beep_sound();
    
    // Start the metronome thread
    let state_clone = Arc::clone(&state);
    let tick_tx_clone = tick_tx.clone();
    let audio_tx_clone = audio_tx.clone();
    
    thread::spawn(move || {
        metronome_loop(state_clone, tick_tx_clone, audio_tx_clone);
    });
    
    // Enable raw mode for better terminal control
    enable_raw_mode()?;
    
    // Main UI loop
    loop {
        display_ui(&state)?;
        
        // Handle audio commands
        if let Ok(cmd) = audio_rx.try_recv() {
            match cmd {
                AudioCommand::PlayTick => {
                    let source = rodio::buffer::SamplesBuffer::new(1, 44100, beep_sound.clone());
                    sink.append(source);
                }
                AudioCommand::Stop => break,
            }
        }
        
        // Check for tick notifications
        if let Ok(_) = tick_rx.try_recv() {
            // Tick received, UI will be updated in next iteration
        }
        
        // Handle user input
        if poll(Duration::from_millis(50))? {
            match read()? {
                Event::Key(key_event) => {
                    // Only handle key press events, not key release
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
        let (should_tick, interval) = {
            let state_guard = state.lock().unwrap();
            if !state_guard.is_running {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            
            let interval = Duration::from_millis(60000 / state_guard.bpm as u64);
            (last_tick.elapsed() >= interval, interval)
        };
        
        if should_tick {
            // Send audio command
            let _ = audio_tx.send(AudioCommand::PlayTick);
            last_tick = Instant::now();
            
            // Update remaining ticks and handle randomization
            {
                let mut state_guard = state.lock().unwrap();
                if state_guard.random_mode {
                    // Ensure remaining_ticks is initialized
                    if state_guard.remaining_ticks == 0 {
                        state_guard.remaining_ticks = state_guard.random_count;
                    }
                    
                    state_guard.remaining_ticks -= 1;
                    
                    if state_guard.remaining_ticks == 0 {
                        // Time to randomize BPM
                        let mut rng = rand::thread_rng();
                        state_guard.bpm = rng.gen_range(60..=200);
                        state_guard.remaining_ticks = state_guard.random_count;
                    }
                }
            }
            
            // Notify UI thread
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
    
    // Current BPM
    execute!(
        io::stdout(),
        SetForegroundColor(Color::Cyan),
        Print(format!("BPM: {}\n", state_guard.bpm)),
        ResetColor,
    )?;
    
    // Status
    let status = if state_guard.is_running { "RUNNING" } else { "STOPPED" };
    let status_color = if state_guard.is_running { Color::Green } else { Color::Red };
    
    execute!(
        io::stdout(),
        Print("Status: "),
        SetForegroundColor(status_color),
        Print(format!("{}\n", status)),
        ResetColor,
    )?;
    
    // Random mode info
    if state_guard.random_mode {
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Yellow),
            Print("🎲 RANDOM MODE ACTIVE\n"),
            ResetColor,
        )?;
        
        // Always show remaining ticks when random mode is active
        let remaining = if state_guard.remaining_ticks == 0 && state_guard.is_running {
            state_guard.random_count // Show what it will be initialized to
        } else {
            state_guard.remaining_ticks
        };
        
        execute!(
            io::stdout(),
            Print(format!("Remaining ticks: {}\n", remaining)),
        )?;
        
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
    println!("  Q         - Quit");
    
    println!("\n💡 Random mode will change BPM every {} ticks", state_guard.random_count);
    
    io::stdout().flush()?;
    
    Ok(())
}

fn toggle_metronome(state: &Arc<Mutex<MetronomeState>>) {
    let mut state_guard = state.lock().unwrap();
    state_guard.is_running = !state_guard.is_running;
    
    // If starting in random mode, initialize remaining ticks
    if state_guard.is_running && state_guard.random_mode && state_guard.remaining_ticks == 0 {
        state_guard.remaining_ticks = state_guard.random_count;
    }
}

fn toggle_random_mode(state: &Arc<Mutex<MetronomeState>>) {
    let mut state_guard = state.lock().unwrap();
    state_guard.random_mode = !state_guard.random_mode;
    
    // Initialize remaining ticks when turning on random mode
    if state_guard.random_mode {
        state_guard.remaining_ticks = state_guard.random_count;
    } else {
        // Reset remaining ticks when turning off random mode
        state_guard.remaining_ticks = 0;
    }
}

fn adjust_bpm(state: &Arc<Mutex<MetronomeState>>, change: i32) {
    let mut state_guard = state.lock().unwrap();
    let new_bpm = (state_guard.bpm as i32 + change).max(30).min(300) as u32;
    state_guard.bpm = new_bpm;
}

fn adjust_random_count(state: &Arc<Mutex<MetronomeState>>) {
    let mut state_guard = state.lock().unwrap();
    let new_count = (state_guard.random_count as i32 + change).max(10).min(1000) as u32;
    state_guard.random_count = new_count;
    
    // If random mode is active and metronome is running, update remaining ticks
    if state_guard.random_mode && state_guard.is_running {
        // Only update if we're increasing the count and current remaining is less than new count
        // This prevents resetting the countdown when you're in the middle of it
        if new_count > state_guard.remaining_ticks {
            state_guard.remaining_ticks = new_count;
        }
    }
}

// Cargo.toml dependencies needed:
/*
[dependencies]
rodio = "0.17"
crossterm = "0.27"
rand = "0.8"
*/