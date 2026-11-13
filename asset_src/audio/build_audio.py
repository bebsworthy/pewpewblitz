#!/usr/bin/env python3
"""Build PewPew Blitz original sound effects and music loops (BRL-0158).

Python 3 standard library synthesis. Every clip is defined below as code: oscillators,
envelopes, filters and note events. The script renders deterministic stereo 16-bit 44.1 kHz WAV
masters and encodes them to Ogg Vorbis with FFmpeg's built-in `vorbis` encoder (bit-exact mode,
no metadata), writing the runtime files to `assets/brawler/audio/original/<name>.ogg`.

Usage, from the repository root:

    python3 asset_src/audio/build_audio.py              # everything
    python3 asset_src/audio/build_audio.py ui-click     # selected clips
    python3 asset_src/audio/build_audio.py --wav DIR    # also keep the WAV masters in DIR
    python3 asset_src/audio/build_audio.py --list       # print the inventory

See README.md beside this script for the inventory and the sound contract.
"""

from __future__ import annotations

import argparse
import math
import shutil
import struct
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

RATE = 44_100
TAU = math.tau
ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "assets" / "brawler" / "audio" / "original"

# ---------------------------------------------------------------------------------------------
# Deterministic primitives
# ---------------------------------------------------------------------------------------------


class Noise:
    """xorshift32 white noise in [-1, 1); seeded per clip so output never depends on run order."""

    def __init__(self, seed: int) -> None:
        self.state = (seed * 2_654_435_761) & 0xFFFFFFFF or 0x9E3779B9

    def __call__(self) -> float:
        x = self.state
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        self.state = x
        return x / 2_147_483_648.0 - 1.0


def buffer(seconds: float) -> list[float]:
    return [0.0] * int(round(seconds * RATE))


def midi(note: float) -> float:
    return 440.0 * 2.0 ** ((note - 69) / 12.0)


def sweep(f0: float, f1: float, duration: float):
    """Exponential frequency glide from f0 to f1 over duration seconds."""
    ratio = f1 / f0

    def freq(t: float) -> float:
        return f0 * ratio ** min(t / duration, 1.0)

    return freq


def const(f: float):
    return lambda _t: f


def wave_value(shape: str, phase: float, duty: float = 0.5) -> float:
    p = phase - math.floor(phase)
    if shape == "sine":
        return math.sin(TAU * p)
    if shape == "square":
        # Offset by the duty cycle's mean so short clips start and end without a DC step.
        return (1.0 if p < duty else -1.0) - (2.0 * duty - 1.0)
    if shape == "saw":
        return 2.0 * p - 1.0
    if shape == "tri":
        return 4.0 * p - 1.0 if p < 0.5 else 3.0 - 4.0 * p
    raise ValueError(shape)


def env(t: float, attack: float, decay: float, length: float, release: float = 0.01) -> float:
    """Attack ramp, exponential decay (time constant `decay`), short release at `length`."""
    if t < 0.0 or t >= length:
        return 0.0
    a = t / attack if attack > 0.0 and t < attack else 1.0
    d = math.exp(-(t - attack) / decay) if t > attack and decay > 0.0 else 1.0
    r = (length - t) / release if release > 0.0 and length - t < release else 1.0
    return a * d * r


def tone(
    out: list[float],
    start: float,
    length: float,
    freq,
    shape: str = "sine",
    gain: float = 1.0,
    attack: float = 0.002,
    decay: float = 1e9,
    release: float = 0.01,
    duty: float = 0.5,
    vibrato: tuple[float, float] = (0.0, 0.0),
    wrap: bool = False,
) -> None:
    """Adds one oscillator voice. With wrap=True the voice wraps around the buffer (loops)."""
    n = len(out)
    first = int(round(start * RATE))
    count = int(round(length * RATE))
    phase = 0.0
    vib_rate, vib_depth = vibrato
    for i in range(count):
        t = i / RATE
        f = freq(t)
        if vib_depth:
            f *= 1.0 + vib_depth * math.sin(TAU * vib_rate * t)
        phase += f / RATE
        index = first + i
        if wrap:
            index %= n
        elif index >= n:
            break
        out[index] += gain * env(t, attack, decay, length, release) * wave_value(shape, phase, duty)


def noise(
    out: list[float],
    start: float,
    length: float,
    seed: int,
    gain: float = 1.0,
    attack: float = 0.001,
    decay: float = 1e9,
    release: float = 0.01,
    lowpass=None,
    highpass=None,
    bandpass=None,
    q: float = 0.7,
    flutter: tuple[float, float] = (0.0, 0.0),
    wrap: bool = False,
) -> None:
    """Adds filtered white noise. Cutoffs may be constants or functions of time (Hz)."""
    rng = Noise(seed)
    n = len(out)
    first = int(round(start * RATE))
    count = int(round(length * RATE))
    lp = svf()
    hp = svf()
    bp = svf()
    rate, depth = flutter
    for i in range(count):
        t = i / RATE
        x = rng()
        if bandpass is not None:
            x = bp(x, _cut(bandpass, t), q, "band")
        if lowpass is not None:
            x = lp(x, _cut(lowpass, t), q, "low")
        if highpass is not None:
            x = hp(x, _cut(highpass, t), q, "high")
        if depth:
            x *= 1.0 - depth * (0.5 + 0.5 * math.sin(TAU * rate * t))
        index = first + i
        if wrap:
            index %= n
        elif index >= n:
            break
        out[index] += gain * env(t, attack, decay, length, release) * x


def _cut(value, t: float) -> float:
    return value(t) if callable(value) else value


def svf():
    """Chamberlin state-variable filter; returns a stateful step(x, cutoff, q, mode)."""
    low = 0.0
    band = 0.0

    def step(x: float, cutoff: float, q: float, mode: str) -> float:
        nonlocal low, band
        f = 2.0 * math.sin(math.pi * min(cutoff, RATE / 6.0) / RATE)
        damp = 1.0 / max(q, 0.5)
        low += f * band
        high = x - low - damp * band
        band += f * high
        if mode == "low":
            return low
        if mode == "high":
            return high
        return band

    return step


def lowpass_in_place(samples: list[float], cutoff, q: float = 0.7, loop: bool = False) -> None:
    """Resonant low-pass. With loop=True a warm-up pass over the buffer carries the filter state
    across the seam, so the filtered loop joins without a click."""
    step = svf()
    if loop:
        for i, x in enumerate(samples):
            step(x, _cut(cutoff, i / RATE), q, "low")
    for i, x in enumerate(samples):
        samples[i] = step(x, _cut(cutoff, i / RATE), q, "low")


def drive(samples: list[float], amount: float) -> None:
    norm = math.tanh(amount)
    for i, x in enumerate(samples):
        samples[i] = math.tanh(amount * x) / norm


def echo(samples: list[float], delay: float, feedback: float, repeats: int) -> list[float]:
    out = samples[:] + [0.0] * int(delay * RATE * repeats)
    step = int(delay * RATE)
    gain = 1.0
    for r in range(1, repeats + 1):
        gain *= feedback
        offset = step * r
        for i, x in enumerate(samples):
            out[i + offset] += gain * x
    return out


def mix(*parts: list[float]) -> list[float]:
    length = max(len(p) for p in parts)
    out = [0.0] * length
    for part in parts:
        for i, x in enumerate(part):
            out[i] += x
    return out


PEAK_CEILING_DB = -1.0


def normalize(samples: list[float], loudness_db: float) -> list[float]:
    """Scales to a target RMS loudness (dBFS), limited so the peak stays under the ceiling."""
    peak = max((abs(x) for x in samples), default=0.0)
    if peak == 0.0:
        return samples
    rms = math.sqrt(sum(x * x for x in samples) / len(samples))
    gain = min(10.0 ** (loudness_db / 20.0) / rms, 10.0 ** (PEAK_CEILING_DB / 20.0) / peak)
    return [x * gain for x in samples]


def fade_tail(samples: list[float], seconds: float = 0.01) -> list[float]:
    count = min(len(samples), int(seconds * RATE))
    for i in range(count):
        samples[-1 - i] *= i / count
    return samples


def trim_silence(samples: list[float], threshold: float = 1e-4) -> list[float]:
    end = len(samples)
    while end > 1 and abs(samples[end - 1]) < threshold:
        end -= 1
    return samples[: end + int(0.005 * RATE)]


# ---------------------------------------------------------------------------------------------
# Sound effects. Each returns mono samples; the builder normalizes and duplicates to stereo.
# ---------------------------------------------------------------------------------------------


def fire_sticky() -> list[float]:
    out = buffer(0.22)
    tone(out, 0.0, 0.2, sweep(420, 110, 0.2), "square", 0.35, decay=0.07, duty=0.3)
    tone(out, 0.0, 0.2, lambda t: 260 + 90 * math.sin(TAU * 38 * t), "sine", 0.6, decay=0.06)
    noise(out, 0.0, 0.06, 11, 0.4, decay=0.015, lowpass=2400)
    return out


def fire_lobbed() -> list[float]:
    out = buffer(0.32)
    tone(out, 0.0, 0.3, sweep(210, 70, 0.25), "sine", 0.9, decay=0.09)
    tone(out, 0.0, 0.3, sweep(420, 140, 0.25), "tri", 0.25, decay=0.05)
    noise(out, 0.0, 0.05, 12, 0.5, decay=0.012, bandpass=900, q=1.2)
    return out


def fire_splash() -> list[float]:
    out = buffer(0.36)
    tone(out, 0.0, 0.25, sweep(190, 80, 0.2), "sine", 0.7, decay=0.07)
    noise(out, 0.02, 0.32, 13, 0.7, attack=0.02, decay=0.09, bandpass=sweep(2600, 500, 0.3), q=2.0)
    tone(out, 0.05, 0.12, sweep(500, 900, 0.12), "sine", 0.2, decay=0.04)
    return out


def fire_melee() -> list[float]:
    out = buffer(0.26)
    noise(
        out,
        0.0,
        0.24,
        14,
        1.0,
        attack=0.06,
        decay=0.07,
        bandpass=lambda t: 500 + 2600 * math.sin(math.pi * min(t / 0.24, 1.0)),
        q=1.6,
    )
    tone(out, 0.0, 0.18, sweep(300, 180, 0.18), "tri", 0.15, attack=0.05, decay=0.06)
    return out


def fire_spray() -> list[float]:
    out = buffer(0.38)
    noise(out, 0.0, 0.36, 15, 1.0, attack=0.015, decay=0.15, lowpass=3200, highpass=500,
          flutter=(32.0, 0.6))
    tone(out, 0.0, 0.3, sweep(140, 100, 0.3), "saw", 0.12, attack=0.01, decay=0.1)
    return out


def impact_lob() -> list[float]:
    out = buffer(0.5)
    tone(out, 0.0, 0.45, sweep(130, 38, 0.35), "sine", 1.0, decay=0.13)
    noise(out, 0.0, 0.4, 21, 0.6, decay=0.08, lowpass=sweep(1800, 200, 0.3))
    s = out
    drive(s, 1.8)
    return s


def impact_melee() -> list[float]:
    out = buffer(0.18)
    noise(out, 0.0, 0.07, 22, 0.9, decay=0.012, highpass=900)
    tone(out, 0.0, 0.16, sweep(230, 70, 0.12), "sine", 0.9, decay=0.04)
    drive(out, 1.5)
    return out


def impact_spray() -> list[float]:
    out = buffer(0.24)
    noise(out, 0.0, 0.22, 23, 1.0, decay=0.06, highpass=2200, flutter=(55.0, 0.8))
    return out


def hit_dealt() -> list[float]:
    out = buffer(0.14)
    tone(out, 0.0, 0.12, const(1320), "tri", 0.6, decay=0.035)
    tone(out, 0.025, 0.11, const(1980), "sine", 0.55, decay=0.03)
    noise(out, 0.0, 0.01, 31, 0.25, decay=0.003, highpass=4000)
    return out


def hit_taken() -> list[float]:
    out = buffer(0.24)
    tone(out, 0.0, 0.22, sweep(170, 55, 0.18), "sine", 1.0, decay=0.06)
    tone(out, 0.0, 0.12, sweep(340, 110, 0.12), "square", 0.18, decay=0.03, duty=0.25)
    noise(out, 0.0, 0.12, 32, 0.45, decay=0.03, lowpass=1100)
    drive(out, 2.2)
    return out


def respawn() -> list[float]:
    out = buffer(0.62)
    for k, note in enumerate((72, 76, 79, 84)):
        tone(out, k * 0.07, 0.35, const(midi(note)), "tri", 0.45, decay=0.12)
        tone(out, k * 0.07, 0.35, const(midi(note + 12)), "sine", 0.12, decay=0.08)
    noise(out, 0.0, 0.5, 33, 0.08, attack=0.2, decay=0.15, highpass=6000)
    return out


def sentry_fire() -> list[float]:
    out = buffer(0.11)
    tone(out, 0.0, 0.1, sweep(1700, 520, 0.09), "square", 0.5, decay=0.03, duty=0.25)
    return out


def sentry_removed() -> list[float]:
    out = buffer(0.6)
    raw = buffer(0.6)
    tone(raw, 0.0, 0.55, sweep(620, 70, 0.5), "saw", 0.8, decay=0.25)
    lowpass_in_place(raw, sweep(4000, 300, 0.5))
    noise(out, 0.0, 0.08, 34, 0.3, decay=0.02, bandpass=1500, q=2)
    return mix(out, raw)


def ultimate_cloak() -> list[float]:
    out = buffer(0.7)
    tone(out, 0.0, 0.65, sweep(900, 180, 0.6), "sine", 0.6, attack=0.02, decay=0.3,
         vibrato=(9.0, 0.04))
    tone(out, 0.0, 0.65, sweep(905, 182, 0.6), "sine", 0.4, attack=0.02, decay=0.3)
    noise(out, 0.0, 0.65, 41, 0.5, attack=0.05, decay=0.25, bandpass=sweep(5000, 600, 0.6), q=3)
    return out


def ultimate_uncloak() -> list[float]:
    out = buffer(0.48)
    tone(out, 0.0, 0.42, sweep(200, 820, 0.4), "sine", 0.6, attack=0.08, decay=0.4)
    noise(out, 0.0, 0.42, 42, 0.45, attack=0.15, decay=0.2, bandpass=sweep(700, 5200, 0.4), q=3)
    return out


def ultimate_scan() -> list[float]:
    ping = buffer(0.3)
    tone(ping, 0.0, 0.28, const(1250), "sine", 0.8, decay=0.08)
    tone(ping, 0.0, 0.28, const(2500), "sine", 0.15, decay=0.04)
    tone(ping, 0.0, 0.05, sweep(400, 1250, 0.05), "tri", 0.3, decay=0.02)
    return echo(ping, 0.16, 0.45, 4)


def ultimate_revealed() -> list[float]:
    out = buffer(0.4)
    tone(out, 0.0, 0.14, const(880), "square", 0.4, decay=0.2, duty=0.4)
    tone(out, 0.18, 0.18, const(660), "square", 0.4, decay=0.2, duty=0.4)
    lowpass_in_place(out, 3500)
    return out


def ultimate_field() -> list[float]:
    out = buffer(0.95)
    noise(out, 0.0, 0.9, 43, 0.7, attack=0.35, decay=0.3, lowpass=sweep(300, 1600, 0.6))
    for note in (45, 52, 57):
        tone(out, 0.0, 0.9, const(midi(note)), "saw", 0.18, attack=0.3, decay=0.35)
    lowpass_in_place(out, 2200)
    return out


def ultimate_strike() -> list[float]:
    out = buffer(1.2)
    tone(out, 0.0, 1.1, sweep(95, 28, 0.8), "sine", 1.0, decay=0.35)
    noise(out, 0.0, 1.1, 44, 0.9, decay=0.3, lowpass=sweep(4000, 180, 0.9))
    noise(out, 0.0, 0.05, 45, 0.6, decay=0.01, highpass=2500)
    drive(out, 2.5)
    return out


def dash() -> list[float]:
    out = buffer(0.27)
    noise(out, 0.0, 0.25, 51, 1.0, attack=0.03, decay=0.08, bandpass=sweep(450, 3600, 0.22), q=2.2)
    return out


def charge_ready() -> list[float]:
    out = buffer(0.85)
    for k, note in enumerate((76, 81, 88)):
        tone(out, k * 0.09, 0.55, const(midi(note)), "tri", 0.4, decay=0.25)
        tone(out, k * 0.09, 0.55, const(midi(note) * 2.005), "sine", 0.12, decay=0.15)
    noise(out, 0.15, 0.6, 52, 0.1, attack=0.1, decay=0.2, highpass=7000)
    return out


def passive() -> list[float]:
    out = buffer(0.17)
    tone(out, 0.0, 0.15, sweep(660, 990, 0.08), "tri", 0.6, decay=0.06)
    return out


def sentry_spawn() -> list[float]:
    out = buffer(0.55)
    for k in range(2):
        noise(out, k * 0.12, 0.05, 53 + k, 0.7, decay=0.012, bandpass=1200, q=2.5)
        tone(out, k * 0.12, 0.06, const(180), "square", 0.25, decay=0.02)
    tone(out, 0.24, 0.28, sweep(220, 520, 0.25), "square", 0.22, decay=0.15, duty=0.3)
    lowpass_in_place(out, 3000)
    return out


def concealment_field_spawn() -> list[float]:
    out = buffer(0.7)
    noise(out, 0.0, 0.66, 55, 1.0, attack=0.2, decay=0.25, lowpass=sweep(400, 2400, 0.4),
          flutter=(7.0, 0.3))
    tone(out, 0.0, 0.66, const(midi(62)), "sine", 0.12, attack=0.2, decay=0.3)
    return out


def reload() -> list[float]:
    out = buffer(0.34)
    noise(out, 0.0, 0.03, 56, 0.9, decay=0.006, highpass=2500)
    noise(out, 0.16, 0.04, 57, 1.0, decay=0.008, highpass=1800)
    tone(out, 0.16, 0.16, const(2200), "sine", 0.25, decay=0.04)
    tone(out, 0.16, 0.16, const(3310), "sine", 0.12, decay=0.03)
    return out


def objective_hit() -> list[float]:
    out = buffer(0.4)
    for freq, gain, decay in ((523, 0.6, 0.12), (1187, 0.35, 0.08), (1789, 0.25, 0.05),
                              (2711, 0.15, 0.03)):
        tone(out, 0.0, 0.38, const(freq), "sine", gain, decay=decay)
    noise(out, 0.0, 0.02, 61, 0.4, decay=0.005, highpass=3000)
    return out


def objective_critical() -> list[float]:
    out = buffer(0.66)
    for k in range(3):
        tone(out, k * 0.2, 0.17, sweep(700, 1050, 0.17), "square", 0.35, decay=0.4, duty=0.4)
    lowpass_in_place(out, 3000)
    return out


def objective_destroyed() -> list[float]:
    out = buffer(1.3)
    tone(out, 0.0, 1.2, sweep(110, 30, 0.9), "sine", 1.0, decay=0.4)
    noise(out, 0.0, 1.2, 62, 0.9, decay=0.35, lowpass=sweep(5000, 250, 1.0))
    for k in range(7):
        noise(out, 0.15 + k * 0.11, 0.05, 63 + k, 0.25 / (1 + k * 0.3), decay=0.01,
              bandpass=2000 + 300 * k, q=3)
    drive(out, 2.0)
    return out


def match_start() -> list[float]:
    out = buffer(1.0)
    for k, note in enumerate((72, 76, 79)):
        tone(out, k * 0.12, 0.11, const(midi(note)), "square", 0.3, decay=0.2, duty=0.25)
    tone(out, 0.36, 0.55, const(midi(84)), "square", 0.32, decay=0.35, duty=0.25,
         vibrato=(6.0, 0.006))
    tone(out, 0.36, 0.55, const(midi(72)), "saw", 0.15, decay=0.35)
    tone(out, 0.36, 0.3, sweep(120, 45, 0.2), "sine", 0.8, decay=0.08)
    lowpass_in_place(out, 5000)
    return out


def match_score() -> list[float]:
    out = buffer(0.42)
    tone(out, 0.0, 0.12, const(midi(79)), "square", 0.3, decay=0.15, duty=0.25)
    tone(out, 0.11, 0.3, const(midi(84)), "square", 0.3, decay=0.15, duty=0.25)
    tone(out, 0.11, 0.3, const(midi(91)), "sine", 0.15, decay=0.1)
    lowpass_in_place(out, 5000)
    return out


def match_victory() -> list[float]:
    out = buffer(2.2)
    lead = ((0.0, 72, 0.14), (0.15, 76, 0.14), (0.3, 79, 0.14), (0.45, 84, 0.3),
            (0.8, 81, 0.14), (0.95, 84, 1.1))
    for start, note, length in lead:
        tone(out, start, length, const(midi(note)), "square", 0.28, decay=0.6, duty=0.25,
             vibrato=(5.5, 0.005))
    for note in (60, 64, 67, 72):
        tone(out, 0.95, 1.2, const(midi(note)), "saw", 0.09, attack=0.02, decay=0.6)
    tone(out, 0.95, 0.4, sweep(110, 40, 0.3), "sine", 0.6, decay=0.12)
    noise(out, 0.95, 1.0, 71, 0.08, decay=0.3, highpass=6000)
    lowpass_in_place(out, 5500)
    return out


def match_defeat() -> list[float]:
    out = buffer(2.0)
    lead = ((0.0, 67, 0.3), (0.32, 65, 0.3), (0.64, 63, 0.3), (0.96, 60, 0.9))
    for start, note, length in lead:
        tone(out, start, length, const(midi(note)), "tri", 0.45, decay=0.5, vibrato=(4.0, 0.006))
    for note in (48, 51, 55):
        tone(out, 0.96, 0.95, const(midi(note)), "saw", 0.08, attack=0.05, decay=0.5)
    lowpass_in_place(out, 2200)
    return out


def match_draw() -> list[float]:
    out = buffer(1.3)
    tone(out, 0.0, 0.3, const(midi(67)), "tri", 0.45, decay=0.4)
    tone(out, 0.32, 0.9, const(midi(67)), "tri", 0.45, decay=0.5)
    for note in (60, 65, 67):
        tone(out, 0.32, 0.9, const(midi(note)), "saw", 0.08, attack=0.05, decay=0.5)
    lowpass_in_place(out, 2600)
    return out


def zone_captured() -> list[float]:
    out = buffer(0.7)
    tone(out, 0.0, 0.16, const(midi(74)), "tri", 0.5, decay=0.3)
    tone(out, 0.15, 0.45, const(midi(81)), "tri", 0.5, decay=0.25)
    tone(out, 0.15, 0.45, const(midi(93)), "sine", 0.18, decay=0.12)
    return out


def zone_lost() -> list[float]:
    out = buffer(0.55)
    tone(out, 0.0, 0.16, const(midi(76)), "tri", 0.5, decay=0.3)
    tone(out, 0.15, 0.38, const(midi(69)), "tri", 0.5, decay=0.2)
    tone(out, 0.15, 0.38, const(midi(68.6)), "tri", 0.2, decay=0.2)
    return out


def zone_progress() -> list[float]:
    out = buffer(0.27)
    tone(out, 0.0, 0.07, const(1480), "sine", 0.6, decay=0.02)
    tone(out, 0.12, 0.09, const(1760), "sine", 0.6, decay=0.025)
    noise(out, 0.0, 0.01, 81, 0.2, decay=0.003, highpass=3000)
    return out


def pickup_spawned() -> list[float]:
    out = buffer(0.4)
    for k, note in enumerate((84, 88, 91, 96)):
        tone(out, k * 0.05, 0.16, sweep(midi(note) * 0.9, midi(note), 0.03), "sine", 0.4,
             decay=0.05)
    return out


def pickup_collected() -> list[float]:
    out = buffer(0.36)
    tone(out, 0.0, 0.07, const(midi(83)), "square", 0.3, decay=0.2, duty=0.5)
    tone(out, 0.065, 0.28, const(midi(88)), "square", 0.3, decay=0.1, duty=0.5)
    lowpass_in_place(out, 6000)
    return out


def ui_click() -> list[float]:
    out = buffer(0.06)
    tone(out, 0.0, 0.05, const(1850), "tri", 0.7, decay=0.012)
    noise(out, 0.0, 0.006, 91, 0.25, decay=0.002, highpass=5000)
    return out


def ui_confirm() -> list[float]:
    out = buffer(0.16)
    tone(out, 0.0, 0.06, const(880), "tri", 0.6, decay=0.04)
    tone(out, 0.055, 0.1, const(1320), "tri", 0.6, decay=0.04)
    return out


def ui_back() -> list[float]:
    out = buffer(0.14)
    tone(out, 0.0, 0.055, const(1100), "tri", 0.55, decay=0.035)
    tone(out, 0.05, 0.085, const(740), "tri", 0.55, decay=0.035)
    return out


# ---------------------------------------------------------------------------------------------
# Music loops. Every event wraps around the loop buffer so tails cross the seam seamlessly.
# ---------------------------------------------------------------------------------------------

A_MINOR_CHORDS = ((57, 60, 64), (53, 57, 60), (48, 52, 55), (55, 59, 62))  # Am F C G


def music_menu() -> list[float]:
    bpm = 100.0
    beat = 60.0 / bpm
    bars = 8
    out = buffer(bars * 4 * beat)
    pad = buffer(bars * 4 * beat)
    for bar in range(bars):
        chord = A_MINOR_CHORDS[bar % 4]
        start = bar * 4 * beat
        for note in chord:
            for detune in (-0.08, 0.08):
                tone(pad, start, 4 * beat + 0.6, const(midi(note + detune)), "saw", 0.06,
                     attack=0.35, decay=6.0, release=0.6, wrap=True)
        tone(out, start, 4 * beat, const(midi(chord[0] - 12)), "sine", 0.32, attack=0.03,
             decay=3.0, release=0.2, wrap=True)
        arp = (chord[0], chord[1], chord[2], chord[1] + 12, chord[2], chord[1], chord[0] + 12,
               chord[2])
        for step, note in enumerate(arp):
            t = start + step * beat / 2
            for tap, gain in ((0.0, 0.16), (0.75 * beat, 0.06), (1.5 * beat, 0.025)):
                tone(out, t + tap, 0.32, const(midi(note + 12)), "square", gain, decay=0.08,
                     duty=0.3, wrap=True)
        for k in (0, 2):
            tone(out, start + k * beat, 0.25, sweep(110, 42, 0.12), "sine", 0.4, decay=0.07,
                 wrap=True)
        for k in range(8):
            if k % 2:
                noise(out, start + k * beat / 2, 0.05, 100 + bar * 8 + k, 0.05, decay=0.012,
                      highpass=7000, wrap=True)
    lowpass_in_place(pad, 1300, loop=True)
    lowpass_in_place(out, 4200, loop=True)
    return mix(out, pad)


D_MINOR_CHORDS = ((50, 53, 57), (46, 50, 53), (48, 52, 55), (45, 48, 52))  # Dm Bb C Am


def music_match() -> list[float]:
    bpm = 128.0
    beat = 60.0 / bpm
    bars = 16
    length = bars * 4 * beat
    drums = buffer(length)
    bass = buffer(length)
    keys = buffer(length)
    melody = (74, 72, 69, 72, 74, 77, 76, 72)
    for bar in range(bars):
        chord = D_MINOR_CHORDS[bar % 4]
        start = bar * 4 * beat
        for k in range(4):
            t = start + k * beat
            tone(drums, t, 0.3, sweep(150, 45, 0.09), "sine", 0.85, decay=0.08, wrap=True)
            noise(drums, t, 0.02, 200 + bar * 4 + k, 0.2, decay=0.004, lowpass=3000, wrap=True)
            if k in (1, 3):
                noise(drums, t, 0.18, 300 + bar * 4 + k, 0.45, decay=0.05, bandpass=1800, q=0.8,
                      wrap=True)
                tone(drums, t, 0.1, sweep(240, 180, 0.08), "tri", 0.25, decay=0.04, wrap=True)
        for k in range(16):
            accent = 0.07 if k % 4 == 2 else 0.035
            noise(drums, start + k * beat / 4, 0.04, 400 + bar * 16 + k, accent, decay=0.01,
                  highpass=8000, wrap=True)
        for k in range(8):
            note = chord[0] - 12 + (12 if k % 2 else 0)
            tone(bass, start + k * beat / 2, beat / 2 - 0.02, const(midi(note)), "saw", 0.28,
                 decay=0.12, release=0.02, wrap=True)
        for k, step in enumerate((0, 1.5, 3)):
            for note in chord:
                tone(keys, start + step * beat, 0.35, const(midi(note + 12)), "square", 0.045,
                     decay=0.12, duty=0.35, wrap=True)
        if bar >= 8:
            phrase = melody if bar % 2 == 0 else tuple(reversed(melody))
            for k, note in enumerate(phrase):
                t = start + k * beat / 2
                tone(keys, t, beat / 2 * 0.9, const(midi(note + 12 * (bar >= 12))), "square",
                     0.09, decay=0.2, duty=0.25, vibrato=(6.0, 0.004), wrap=True)
                tone(keys, t + 0.75 * beat, beat / 2 * 0.9, const(midi(note)), "square", 0.03,
                     decay=0.2, duty=0.25, wrap=True)
    lowpass_in_place(bass, 900, q=1.2, loop=True)
    lowpass_in_place(keys, 3800, loop=True)
    return mix(drums, bass, keys)


# ---------------------------------------------------------------------------------------------
# Inventory and builder
# ---------------------------------------------------------------------------------------------

# name: (render function, target RMS loudness dBFS (peak-limited to -1 dBFS), loop)
CLIPS = {
    "fire-sticky": (fire_sticky, -17.0, False),
    "fire-lobbed": (fire_lobbed, -17.0, False),
    "fire-splash": (fire_splash, -17.0, False),
    "fire-melee": (fire_melee, -18.0, False),
    "fire-spray": (fire_spray, -19.0, False),
    "impact-lob": (impact_lob, -15.0, False),
    "impact-melee": (impact_melee, -16.0, False),
    "impact-spray": (impact_spray, -20.0, False),
    "hit-dealt": (hit_dealt, -17.0, False),
    "hit-taken": (hit_taken, -15.0, False),
    "respawn": (respawn, -18.0, False),
    "sentry-fire": (sentry_fire, -20.0, False),
    "sentry-removed": (sentry_removed, -18.0, False),
    "ultimate-cloak": (ultimate_cloak, -17.0, False),
    "ultimate-uncloak": (ultimate_uncloak, -17.0, False),
    "ultimate-scan": (ultimate_scan, -18.0, False),
    "ultimate-revealed": (ultimate_revealed, -17.0, False),
    "ultimate-field": (ultimate_field, -16.0, False),
    "ultimate-strike": (ultimate_strike, -13.0, False),
    "dash": (dash, -18.0, False),
    "charge-ready": (charge_ready, -17.0, False),
    "passive": (passive, -20.0, False),
    "sentry-spawn": (sentry_spawn, -18.0, False),
    "concealment-field-spawn": (concealment_field_spawn, -18.0, False),
    "reload": (reload, -21.0, False),
    "objective-hit": (objective_hit, -18.0, False),
    "objective-critical": (objective_critical, -16.0, False),
    "objective-destroyed": (objective_destroyed, -13.0, False),
    "match-start": (match_start, -15.0, False),
    "match-score": (match_score, -17.0, False),
    "match-victory": (match_victory, -15.0, False),
    "match-defeat": (match_defeat, -15.0, False),
    "match-draw": (match_draw, -16.0, False),
    "zone-captured": (zone_captured, -16.0, False),
    "zone-lost": (zone_lost, -16.0, False),
    "zone-progress": (zone_progress, -19.0, False),
    "pickup-spawned": (pickup_spawned, -19.0, False),
    "pickup-collected": (pickup_collected, -17.0, False),
    "ui-click": (ui_click, -24.0, False),
    "ui-confirm": (ui_confirm, -22.0, False),
    "ui-back": (ui_back, -22.0, False),
    "music-menu": (music_menu, -19.0, True),
    "music-match": (music_match, -18.0, True),
}


def render(name: str) -> list[float]:
    function, loudness_db, loop = CLIPS[name]
    samples = function()
    if not loop:
        samples = fade_tail(trim_silence(samples))
    return normalize(samples, loudness_db)


def write_wav(path: Path, samples: list[float]) -> None:
    frames = bytearray()
    for x in samples:
        value = int(round(max(-1.0, min(1.0, x)) * 32767))
        frames += struct.pack("<hh", value, value)
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(2)
        handle.setsampwidth(2)
        handle.setframerate(RATE)
        handle.writeframes(bytes(frames))


def encode(source: Path, target: Path) -> None:
    ffmpeg = shutil.which("ffmpeg")
    if ffmpeg is None:
        sys.exit("ffmpeg is required to encode Ogg Vorbis (see README.md)")
    subprocess.run(
        [
            ffmpeg, "-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i", str(source),
            "-c:a", "vorbis", "-strict", "-2", "-q:a", "4", "-map_metadata", "-1",
            "-fflags", "+bitexact", "-flags:a", "+bitexact", str(target),
        ],
        check=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("names", nargs="*", help="clip names to build (default: all)")
    parser.add_argument("--wav", type=Path, help="also keep WAV masters in this directory")
    parser.add_argument("--list", action="store_true", help="print the inventory and exit")
    args = parser.parse_args()
    if args.list:
        for name, (_, _, loop) in CLIPS.items():
            print(f"{name}{' (loop)' if loop else ''}")
        return
    names = args.names or list(CLIPS)
    unknown = [name for name in names if name not in CLIPS]
    if unknown:
        sys.exit(f"unknown clip(s): {', '.join(unknown)}")
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if args.wav:
        args.wav.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as scratch:
        for name in names:
            samples = render(name)
            master = (args.wav or Path(scratch)) / f"{name}.wav"
            write_wav(master, samples)
            target = OUT_DIR / f"{name}.ogg"
            encode(master, target)
            print(f"{name}: {len(samples) / RATE:.3f} s, {target.stat().st_size} bytes")
    if not args.names:
        stale = sorted(p for p in OUT_DIR.glob("*.ogg") if p.stem not in CLIPS)
        for path in stale:
            path.unlink()
            print(f"removed stale {path.name}")


if __name__ == "__main__":
    main()
