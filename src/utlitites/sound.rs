use std::f32::consts::PI;
use rand::Rng;

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
