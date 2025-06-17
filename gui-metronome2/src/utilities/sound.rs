use rand::Rng;
use std::f32::consts::PI;

pub fn create_click_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 10;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 50.0).exp();
        let sample = (t * 2000.0 * 2.0 * PI).sin() * envelope * 0.5;
        wave.push(sample);
    }
    wave
}

pub fn create_wood_block_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 80;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 15.0).exp();

        let freq1 = 1200.0;
        let freq2 = 800.0;
        let sample1 = (t * freq1 * 2.0 * PI).sin() * 0.3;
        let sample2 = (t * freq2 * 2.0 * PI).sin() * 0.2;
        let sample = (sample1 + sample2) * envelope;
        wave.push(sample);
    }
    wave
}

pub fn create_cowbell_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 120;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 8.0).exp();

        let fundamental = 800.0;
        let sample = ((t * fundamental * 2.0 * PI).sin() * 0.4
            + (t * fundamental * 2.4 * 2.0 * PI).sin() * 0.3
            + (t * fundamental * 3.2 * 2.0 * PI).sin() * 0.2
            + (t * fundamental * 4.1 * 2.0 * PI).sin() * 0.1)
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

pub fn create_hihat_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_ms = 60;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    let mut rng = rand::thread_rng();

    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let envelope = (-t * 25.0).exp();

        let noise: f32 = rng.gen_range(-1.0..1.0);
        let filtered_noise = noise * envelope * 0.3;

        let high_freq = (t * 8000.0 * 2.0 * PI).sin() * envelope * 0.1;

        let sample = filtered_noise + high_freq;
        wave.push(sample);
    }
    wave
}

pub fn create_triangle_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let frequency = 800.0;
    let duration_ms = 80;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let phase = (t * frequency) % 1.0;

        let sample = if phase < 0.5 {
            4.0 * phase - 1.0
        } else {
            3.0 - 4.0 * phase
        } * 0.3;

        wave.push(sample);
    }
    wave
}

pub fn create_square_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let frequency = 600.0;
    let duration_ms = 60;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let phase = (t * frequency) % 1.0;

        let envelope = (-t * 10.0).exp();
        let sample = if phase < 0.5 { 1.0 } else { -1.0 } * 0.3 * envelope;
        wave.push(sample);
    }
    wave
}

pub fn create_beep_sound() -> Vec<f32> {
    let sample_rate = 44100;
    let frequency = 800.0;
    let duration_ms = 50;
    let samples = (sample_rate * duration_ms / 1000) as usize;

    let mut wave: Vec<f32> = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * frequency * 2.0 * PI).sin() * 0.3;
        wave.push(sample);
    }
    wave
}

// pub fn create_celebration_sound() -> Vec<f32> {
//     let sample_rate = 44100;
//     let duration = 2.0; // 2 seconds
//     let mut samples = Vec::new();
    
//     // Create a celebratory chord progression
//     let frequencies = [
//         [523.25, 659.25, 783.99], // C major chord
//         [587.33, 739.99, 880.0],  // D major chord
//         [659.25, 830.61, 987.77], // E major chord
//         [698.46, 880.0, 1046.5],  // F major chord
//     ];
    
//     for chord_idx in 0..frequencies.len() {
//         let chord_duration = duration / frequencies.len() as f32;
//         let chord_samples = (sample_rate as f32 * chord_duration) as usize;
        
//         for i in 0..chord_samples {
//             let t = i as f32 / sample_rate as f32;
//             let mut sample = 0.0;
            
//             // Add each note in the chord
//             for &freq in &frequencies[chord_idx] {
//                 sample += (t * freq * 2.0 * PI).sin() * 0.2;
//             }
            
//             // Add some envelope
//             let envelope = if t < 0.1 {
//                 t / 0.1
//             } else if t > chord_duration - 0.1 {
//                 (chord_duration - t) / 0.1
//             } else {
//                 1.0
//             };
            
//             samples.push(sample * envelope);
//         }
//     }
    
//     samples
// }


























pub fn create_celebration_sound() -> Vec<f32> {
    const SAMPLE_RATE: f32 = 44100.0;
    const DURATION: f32 = 1.5;
    const CHORD_COUNT: usize = 4;
    
    // Shorter, punchy chord progression
    let chord_progression = [
        [261.63, 329.63, 392.0, 493.88],   // C major 7
        [349.23, 440.0, 523.25, 659.25],   // F major 7
        [392.0, 493.88, 587.33, 739.99],   // G major 7
        [523.25, 659.25, 783.99, 987.77],  // C major (octave higher)
    ];
    
    let total_samples = (SAMPLE_RATE * DURATION) as usize;
    let mut samples = vec![0.0; total_samples];
    
    // Generate each chord with overlapping for smoother transitions
    for (chord_idx, chord) in chord_progression.iter().enumerate() {
        let chord_start = (chord_idx as f32 * DURATION / CHORD_COUNT as f32 * SAMPLE_RATE) as usize;
        let chord_duration = DURATION / CHORD_COUNT as f32 * 1.2; // 20% overlap
        let chord_samples = (SAMPLE_RATE * chord_duration) as usize;
        
        for i in 0..chord_samples {
            let sample_idx = chord_start + i;
            if sample_idx >= total_samples {
                break;
            }
            
            let t = i as f32 / SAMPLE_RATE;
            let mut chord_sample = 0.0;
            
            // Generate each note in the chord with harmonic richness
            for (note_idx, &frequency) in chord.iter().enumerate() {
                let amplitude = 0.15 / chord.len() as f32; // Normalize by chord size
                
                // Add fundamental frequency
                chord_sample += (t * frequency * 2.0 * PI).sin() * amplitude;
                
                // Add subtle harmonics for richness
                chord_sample += (t * frequency * 4.0 * PI).sin() * amplitude * 0.1;
                chord_sample += (t * frequency * 6.0 * PI).sin() * amplitude * 0.05;
                
                // Add slight detuning for natural sound
                let detune = 1.0 + (note_idx as f32 * 0.002);
                chord_sample += (t * frequency * detune * 2.0 * PI).sin() * amplitude * 0.3;
            }
            
            // Enhanced envelope with attack, sustain, and release
            let envelope = calculate_envelope(t, chord_duration);
            
            // Add some sparkle with high-frequency content
            let sparkle = (t * 2000.0 * 2.0 * PI).sin() * 0.02 * envelope * (t * 10.0).sin().abs();
            
            // Blend with existing audio (for overlapping chords)
            let final_sample = (chord_sample + sparkle) * envelope;
            samples[sample_idx] += final_sample;
        }
    }
    
    // Add celebratory "bell" hits at the end
    add_bell_flourish(&mut samples, SAMPLE_RATE, DURATION);
    
    // Apply gentle compression to prevent clipping
    apply_soft_limiter(&mut samples);
    
    samples
}

fn calculate_envelope(t: f32, duration: f32) -> f32 {
    const ATTACK_TIME: f32 = 0.05;
    const RELEASE_TIME: f32 = 0.3;
    
    if t < ATTACK_TIME {
        // Smooth attack
        let progress = t / ATTACK_TIME;
        progress * progress // Quadratic curve for smooth start
    } else if t > duration - RELEASE_TIME {
        // Exponential decay for natural release
        let release_progress = (duration - t) / RELEASE_TIME;
        release_progress * release_progress
    } else {
        // Sustain with slight vibrato
        1.0 + (t * 6.0 * PI).sin() * 0.05
    }
}

fn add_bell_flourish(samples: &mut [f32], sample_rate: f32, duration: f32) {
    let bell_frequencies = [1046.5, 1318.5, 1567.98, 2093.0]; // High C, E, G, C
    let bell_start = (duration * 0.75 * sample_rate) as usize;
    let bell_duration = 0.25;
    let bell_samples = (sample_rate * bell_duration) as usize;
    
    for (i, &freq) in bell_frequencies.iter().enumerate() {
        let delay = i * (sample_rate * 0.05) as usize; // Stagger the bells
        
        for j in 0..bell_samples {
            let sample_idx = bell_start + delay + j;
            if sample_idx >= samples.len() {
                break;
            }
            
            let t = j as f32 / sample_rate;
            let bell_envelope = (-t * 8.0).exp(); // Sharp attack, exponential decay
            let bell_sample = (t * freq * 2.0 * PI).sin() * 0.1 * bell_envelope;
            
            samples[sample_idx] += bell_sample;
        }
    }
}

fn apply_soft_limiter(samples: &mut [f32]) {
    const THRESHOLD: f32 = 0.8;
    
    for sample in samples.iter_mut() {
        if sample.abs() > THRESHOLD {
            *sample = sample.signum() * (THRESHOLD + (sample.abs() - THRESHOLD) * 0.2);
        }
    }
}