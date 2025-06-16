# Use Ubuntu as base for better audio support
FROM ubuntu:22.04

# Prevent interactive prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libasound2-dev \
    libasound2-plugins \
    alsa-utils \
    pulseaudio \
    pulseaudio-utils \
    software-properties-common \
    && rm -rf /var/lib/apt/lists/*

# Add PipeWire PPA for Ubuntu 22.04 and install PipeWire
RUN apt-get update && apt-get install -y \
    pipewire \
    pipewire-pulse \
    && rm -rf /var/lib/apt/lists/* \
    || echo "PipeWire packages not available, continuing with PulseAudio/ALSA only"

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Set working directory
WORKDIR /app

# Create the utilities module directory structure
RUN mkdir -p src/utlitites

# Copy your source files
COPY src/main.rs src/
COPY src/utlitites/ src/utlitites/
COPY Cargo.toml .
COPY Cargo.lock .

# If you don't have a Cargo.toml, here's a basic one that should work:
# You can uncomment and modify this if needed
RUN if [ ! -f Cargo.toml ]; then \
    echo '[package]' > Cargo.toml && \
    echo 'name = "cli-metronome"' >> Cargo.toml && \
    echo 'version = "0.1.0"' >> Cargo.toml && \
    echo 'edition = "2021"' >> Cargo.toml && \
    echo '' >> Cargo.toml && \
    echo '[dependencies]' >> Cargo.toml && \
    echo 'rodio = "0.17"' >> Cargo.toml && \
    echo 'crossterm = "0.27"' >> Cargo.toml && \
    echo 'rand = "0.8"' >> Cargo.toml; \
    fi

# Build the application
RUN cargo build --release

# Copy the built binary to a location in PATH
RUN cp /app/target/release/cli-metronome /usr/local/bin/cli-metronome

# Create a non-root user (optional, for security)
RUN useradd -m -s /bin/bash metronome
USER metronome
WORKDIR /home/metronome

# Set up audio environment for PipeWire
ENV XDG_RUNTIME_DIR=/tmp/runtime
ENV PIPEWIRE_RUNTIME_DIR=/tmp/runtime
ENV PULSE_RUNTIME_PATH=/tmp/runtime/pulse
ENV PULSE_STATE_PATH=/tmp/runtime/pulse-state
ENV ALSA_PCM_CARD=0
ENV ALSA_PCM_DEVICE=0

# Create startup script with audio setup
RUN echo '#!/bin/bash' > start_metronome.sh && \
    echo 'echo "Starting CLI Metronome..."' >> start_metronome.sh && \
    echo 'echo "Checking audio setup..."' >> start_metronome.sh && \
    echo '' >> start_metronome.sh && \
    echo '# Create runtime directory' >> start_metronome.sh && \
    echo 'mkdir -p /tmp/runtime' >> start_metronome.sh && \
    echo '' >> start_metronome.sh && \
    echo '# Try to detect available audio systems' >> start_metronome.sh && \
    echo 'if [ -S "/tmp/runtime/pipewire-0" ]; then' >> start_metronome.sh && \
    echo '    echo "✓ PipeWire detected"' >> start_metronome.sh && \
    echo 'elif pulseaudio --check -v 2>/dev/null; then' >> start_metronome.sh && \
    echo '    echo "✓ PulseAudio detected"' >> start_metronome.sh && \
    echo 'elif aplay -l &>/dev/null; then' >> start_metronome.sh && \
    echo '    echo "✓ ALSA detected"' >> start_metronome.sh && \
    echo 'else' >> start_metronome.sh && \
    echo '    echo "⚠ No audio system detected - running in silent mode"' >> start_metronome.sh && \
    echo 'fi' >> start_metronome.sh && \
    echo '' >> start_metronome.sh && \
    echo 'echo "Controls:"' >> start_metronome.sh && \
    echo 'echo "  SPACE - Start/Stop"' >> start_metronome.sh && \
    echo 'echo "  Q - Quit"' >> start_metronome.sh && \
    echo 'echo "  R - Random mode"' >> start_metronome.sh && \
    echo 'echo "  S/A - Change sounds"' >> start_metronome.sh && \
    echo 'echo "  Arrow keys - Adjust BPM"' >> start_metronome.sh && \
    echo 'echo ""' >> start_metronome.sh && \
    echo 'cli-metronome' >> start_metronome.sh && \
    chmod +x start_metronome.sh

# Default command
CMD ["./start_metronome.sh"]