use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyEventKind, poll, read},
    execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};
use rand::Rng;
use rodio::{OutputStream, Sink};
use std::io::{self, BufWriter, Write};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::utilities::{
    cache::{SoundCache, UICache},
    display::display_enhanced_ui,
    state::AtomicState,
};
mod utilities;

enum AudioCommand {
    PlayTick(Vec<f32>),
    Stop,
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
        metronome_loop(
            state_clone,
            sound_cache_clone,
            tick_tx_clone,
            audio_tx_clone,
        );
    });

    enable_raw_mode()?;
    execute!(io::stdout(), cursor::Hide, Clear(ClearType::All))?;

    let stdout = io::stdout();
    let mut buffered_stdout = BufWriter::new(stdout);

    let mut last_ui_update = Instant::now();
    const UI_UPDATE_INTERVAL: Duration = Duration::from_millis(16);

    let mut input_check_time = Instant::now();
    const INPUT_CHECK_INTERVAL: Duration = Duration::from_millis(8);

    loop {
        let now = Instant::now();

        let should_update_ui = now.duration_since(last_ui_update) >= UI_UPDATE_INTERVAL
            || state.ui_dirty.load(Ordering::Relaxed);

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
                                KeyCode::Char('+') | KeyCode::Char('=') => {
                                    adjust_random_count(&state, 10)
                                }
                                KeyCode::Char('-') | KeyCode::Char('_') => {
                                    adjust_random_count(&state, -10)
                                }
                                KeyCode::Char('s') | KeyCode::Char('n') => {
                                    cycle_sound(&state, true)
                                }
                                KeyCode::Char('a') | KeyCode::Char('p') => {
                                    cycle_sound(&state, false)
                                }
                                KeyCode::Char('t') => {
                                    test_current_sound(&state, &sound_cache, &audio_tx);
                                    needs_ui_update = false;
                                }
                                KeyCode::Char('v') => adjust_volume(&state, 10),
                                KeyCode::Char('c') => adjust_volume(&state, -10),
                                KeyCode::F(1) => set_preset_bpm(&state, 60),
                                KeyCode::F(2) => set_preset_bpm(&state, 120),
                                KeyCode::F(3) => set_preset_bpm(&state, 180),
                                KeyCode::F(4) => set_preset_bpm(&state, 200),
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

    println!("\n* ======================================= *");
    println!("   Thank you for using CLI Metronome!");
    println!("   Keep the rhythm alive! ♪");
    println!("* ======================================= *\n");

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
                    let new_bpm = rng.gen_range(60..=150); // min/max random value
                    state.bpm.store(new_bpm, Ordering::Relaxed);
                    state.remaining_ticks.store(
                        state.random_count.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
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

    if !was_running
        && state.random_mode.load(Ordering::Relaxed)
        && state.remaining_ticks.load(Ordering::Relaxed) == 0
    {
        state.remaining_ticks.store(
            state.random_count.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
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
            state.remaining_ticks.store(
                state.random_count.load(Ordering::Relaxed),
                Ordering::Relaxed,
            );
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
    let new_sound = if forward {
        current.next()
    } else {
        current.prev()
    };
    state.set_sound_type(new_sound);
    state.ui_dirty.store(true, Ordering::Relaxed);
}

fn test_current_sound(
    state: &Arc<AtomicState>,
    sound_cache: &Arc<SoundCache>,
    audio_tx: &mpsc::Sender<AudioCommand>,
) {
    let sound_type = state.get_sound_type();
    let mut sound_data = sound_cache.get_sound(sound_type).clone();

    let volume = state.volume.load(Ordering::Relaxed) as f32 / 100.0;
    for sample in &mut sound_data {
        *sample *= volume;
    }

    let _ = audio_tx.send(AudioCommand::PlayTick(sound_data));
}
