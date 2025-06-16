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

use crate::utlitites::sound::{create_beep_sound, create_kick_sound, create_click_sound, create_cowbell_sound, create_hihat_sound, create_square_sound, create_triangle_sound,};
mod utlitites;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(Mutex::new(MetronomeState::new()));
    
    let (tick_tx, tick_rx) = mpsc::channel();
    let (audio_tx, audio_rx) = mpsc::channel::<AudioCommand>();
    
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    let beep_sound = create_kick_sound();
    
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
                AudioCommand::PlayTick => {
                    let source = rodio::buffer::SamplesBuffer::new(1, 44100, beep_sound.clone());
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
    
    // Initialize remaining ticks when turning on random mode, but only if metronome is running
    if state_guard.random_mode {
        if state_guard.is_running {
            state_guard.remaining_ticks = state_guard.random_count;
        }
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

fn adjust_random_count(state: &Arc<Mutex<MetronomeState>>, change: i32) {
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


// use std::f32::consts::PI;

// // Option 1: Classic metronome click (short, sharp sound)
// fn create_click_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let duration_ms = 10; // Very short for sharp click
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         // High frequency with quick decay
//         let envelope = (-t * 50.0).exp(); // Exponential decay
//         let sample = (t * 2000.0 * 2.0 * PI).sin() * envelope * 0.5;
//         wave.push(sample);
//     }
//     wave
// }

// // Option 2: Wood block sound (two frequencies)
// fn create_wood_block_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let duration_ms = 80;
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         let envelope = (-t * 15.0).exp();
        
//         // Mix two frequencies for wood-like sound
//         let freq1 = 1200.0;
//         let freq2 = 800.0;
//         let sample1 = (t * freq1 * 2.0 * PI).sin() * 0.3;
//         let sample2 = (t * freq2 * 2.0 * PI).sin() * 0.2;
//         let sample = (sample1 + sample2) * envelope;
//         wave.push(sample);
//     }
//     wave
// }

// // Option 3: Cowbell sound
// fn create_cowbell_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let duration_ms = 120;
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         let envelope = (-t * 8.0).exp();
        
//         // Multiple harmonics for metallic sound
//         let fundamental = 800.0;
//         let sample = (
//             (t * fundamental * 2.0 * PI).sin() * 0.4 +
//             (t * fundamental * 2.4 * 2.0 * PI).sin() * 0.3 +
//             (t * fundamental * 3.2 * 2.0 * PI).sin() * 0.2 +
//             (t * fundamental * 4.1 * 2.0 * PI).sin() * 0.1
//         ) * envelope;
//         wave.push(sample);
//     }
//     wave
// }

// // Option 4: Kick drum sound
// fn create_kick_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let duration_ms = 150;
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         let envelope = (-t * 12.0).exp();
        
//         // Low frequency with pitch sweep
//         let freq = 60.0 * (-t * 10.0).exp(); // Frequency drops quickly
//         let sample = (t * freq * 2.0 * PI).sin() * envelope * 0.6;
//         wave.push(sample);
//     }
//     wave
// }

// // Option 5: Hi-hat sound (noise-based)
// fn create_hihat_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let duration_ms = 60;
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     let mut rng = rand::thread_rng();
    
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         let envelope = (-t * 25.0).exp();
        
//         // White noise filtered through high frequencies
//         let noise: f32 = rng.gen_range(-1.0..1.0);
//         let filtered_noise = noise * envelope * 0.3;
        
//         // Add some high frequency content
//         let high_freq = (t * 8000.0 * 2.0 * PI).sin() * envelope * 0.1;
        
//         let sample = filtered_noise + high_freq;
//         wave.push(sample);
//     }
//     wave
// }

// // Option 6: Sine wave with different frequency (original but customizable)
// fn create_custom_beep_sound(frequency: f32, duration_ms: u32, volume: f32) -> Vec<f32> {
//     let sample_rate = 44100;
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         let sample = (t * frequency * 2.0 * PI).sin() * volume;
//         wave.push(sample);
//     }
//     wave
// }

// // Option 7: Triangle wave (softer than sine)
// fn create_triangle_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let frequency = 800.0;
//     let duration_ms = 80;
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         let phase = (t * frequency) % 1.0;
        
//         // Triangle wave
//         let sample = if phase < 0.5 {
//             4.0 * phase - 1.0
//         } else {
//             3.0 - 4.0 * phase
//         } * 0.3;
        
//         wave.push(sample);
//     }
//     wave
// }

// // Option 8: Square wave (more digital/electronic sound)
// fn create_square_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let frequency = 600.0;
//     let duration_ms = 60;
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         let phase = (t * frequency) % 1.0;
        
//         // Square wave with envelope
//         let envelope = (-t * 10.0).exp();
//         let sample = if phase < 0.5 { 1.0 } else { -1.0 } * 0.3 * envelope;
//         wave.push(sample);
//     }
//     wave
// }

// fn create_beep_soundcreate_beep_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let frequency = 800.0;
//     let duration_ms = 50;
//     let samples = (sample_rate * duration_ms / 1000) as usize;
    
//     let mut wave: Vec<f32> = Vec::with_capacity(samples);
//     for i in 0..samples {
//         let t = i as f32 / sample_rate as f32;
//         let sample = (t * frequency * 2.0 * std::f32::consts::PI).sin() * 0.3;
//         wave.push(sample);
//     }
//     wave
// }