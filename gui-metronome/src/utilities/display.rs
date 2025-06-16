use crossterm::{
    cursor, execute,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor},
    terminal::{Clear, ClearType},
};

use std::io::{BufWriter, Stdout, Write};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::utilities::{cache::UICache, sound_type::SoundType, state::AtomicState};

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

pub fn display_enhanced_ui(
    state: &Arc<AtomicState>,
    ui_cache: &Arc<Mutex<UICache>>,
    writer: &mut BufWriter<Stdout>,
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

        let bpm_progress = (current_bpm - 30) as f64 / (300 - 30) as f64;
        let meter = create_progress_bar(bpm_progress, 20, '#', '.');
        execute!(
            writer,
            cursor::MoveTo(12, BPM_PANEL_ROW + 2),
            SetForegroundColor(if current_bpm > 150 {
                Color::Red
            } else if current_bpm > 100 {
                Color::Yellow
            } else {
                Color::Green
            }),
            Print(&meter),
            ResetColor,
        )?;

        cache.last_bpm = current_bpm;
    }

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
            Print(&format!(
                "{} {}",
                current_sound.icon(),
                current_sound.name()
            )),
            ResetColor,
        )?;

        cache.last_sound = current_sound;
    }

    if current_status != cache.last_status
        || current_tick_count != cache.last_tick_count
        || cache.first_render
    {
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
                Print(&format!(
                    "▶️  PLAYING • Beat #{} • {}/4",
                    current_tick_count, beats_per_measure
                )),
                ResetColor,
            )?;

            execute!(writer, cursor::MoveTo(12, STATUS_PANEL_ROW + 2))?;
            for i in 1..=4 {
                if i <= beats_per_measure {
                    execute!(
                        writer,
                        SetForegroundColor(Color::Green),
                        Print("* "),
                        ResetColor
                    )?;
                } else {
                    execute!(
                        writer,
                        SetForegroundColor(Color::DarkGrey),
                        Print("o "),
                        ResetColor
                    )?;
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

    if current_random_mode != cache.last_random_mode
        || current_remaining_ticks != cache.last_remaining_ticks
        || cache.first_render
    {
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

        execute!(
            writer,
            cursor::MoveTo(15, FOOTER_ROW),
            SetForegroundColor(Color::DarkGrey),
            Print(
                "💡 Pro tip: Use random mode for practice • F-keys for quick BPM presets • V/C for volume"
            ),
            ResetColor,
        )?;
    }

    writer.flush()?;
    Ok(())
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

    let beats_per_measure = 4;
    let marker_spacing = ANIMATION_WIDTH / beats_per_measure;
    for i in 0..beats_per_measure {
        let pos = i * marker_spacing;
        if pos < ANIMATION_WIDTH {
            let measure_num = (tick_count as usize / beats_per_measure) % 4;
            animation[pos] = match measure_num {
                0 => '|',
                1 => ':',
                2 => '|',
                3 => ':',
                _ => '|',
            };
        }
    }

    let tick_pos = (progress * (ANIMATION_WIDTH - 1) as f64) as usize;
    if tick_pos < ANIMATION_WIDTH {
        let pulse_index = (tick_count as usize) % PULSE_SYMBOLS.len();
        animation[tick_pos] = PULSE_SYMBOLS[pulse_index];
    }

    let emphasis_duration = Duration::from_millis(150);
    if elapsed < emphasis_duration {
        let fade_progress = elapsed.as_millis() as f64 / emphasis_duration.as_millis() as f64;
        let intensity = ((1.0 - fade_progress) * 4.0) as usize;

        for i in 0..=intensity {
            if tick_pos >= i && tick_pos + i < ANIMATION_WIDTH {
                if i == 0 {
                    animation[tick_pos] = '*';
                } else if i <= 2 {
                    if tick_pos >= i {
                        animation[tick_pos - i] = 'o';
                    }
                    if tick_pos + i < ANIMATION_WIDTH {
                        animation[tick_pos + i] = 'o';
                    }
                }
            }
        }
    }

    format!("🎼 {}", animation.into_iter().collect::<String>())
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

fn draw_box_border(
    writer: &mut BufWriter<Stdout>,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    execute!(writer, cursor::MoveTo(x, y), Print("+"))?;
    for _ in 1..width - 1 {
        execute!(writer, Print("-"))?;
    }
    execute!(writer, Print("+"))?;

    for i in 1..height - 1 {
        execute!(writer, cursor::MoveTo(x, y + i), Print("|"))?;
        execute!(writer, cursor::MoveTo(x + width - 1, y + i), Print("|"))?;
    }

    execute!(writer, cursor::MoveTo(x, y + height - 1), Print("+"))?;
    for _ in 1..width - 1 {
        execute!(writer, Print("-"))?;
    }
    execute!(writer, Print("+"))?;

    Ok(())
}
