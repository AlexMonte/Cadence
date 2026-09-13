#!/usr/bin/env python3
"""Convert one downloaded WebAudioFont preset into a Musaic instrument bank.

Only data literals are parsed; the source JavaScript is never executed. Python
3 is required. Musaic decodes encoded zones during import. See SAMPLE_BANKS.md.
"""
import argparse
import ast
import base64
import json
import math
from pathlib import Path
import re
import struct
import tempfile
import urllib.request

MAX_BYTES = 64 * 1024 * 1024


def read_source(source):
    if source.startswith("https://"):
        with urllib.request.urlopen(source, timeout=30) as response:
            data = response.read(MAX_BYTES + 1)
    else:
        with Path(source).open("rb") as file:
            data = file.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        raise ValueError("A preset source is limited to 64 MiB")
    return data.decode("utf-8")


def parse_preset(source):
    """Parse the object assigned to a preset variable, rejecting expressions."""
    assignment = re.search(r"\bvar\s+[A-Za-z_$][\w$]*\s*=\s*", source)
    if not assignment:
        # Plain exported JSON is also accepted.
        return json.loads(source)
    source = source[assignment.end():]
    token = re.compile(
        r'''\s+|//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|'''
        r'''[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?|[A-Za-z_$][\w$]*|[{}\[\]:,]'''
    )
    position, depth, result = 0, 0, []
    while position < len(source):
        match = token.match(source, position)
        if not match:
            raise ValueError("Preset contains an unsupported expression; only data literals are accepted")
        value = match.group()
        position = match.end()
        if value.isspace() or value.startswith(("//", "/*")):
            continue
        if value in ("{", "["):
            depth += 1
            if depth > 32:
                raise ValueError("Preset is nested too deeply")
        elif value in ("}", "]"):
            depth -= 1
            if result and result[-1] == ",":
                result.pop()
        elif value.startswith(("'", '"')):
            value = json.dumps(ast.literal_eval(value))
        elif re.fullmatch(r"[A-Za-z_$][\w$]*", value) and value not in ("true", "false", "null"):
            value = json.dumps(value)
        # Some distributed presets express cents as 4200-140. Accept only
        # numeric addition/subtraction, never names, calls or general JS.
        if re.fullmatch(r"[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?", value):
            if value.startswith(("+", "-")) and result and re.fullmatch(r"-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?", result[-1]):
                value = json.dumps(float(result.pop()) + float(value))
            else:
                value = json.dumps(float(value))
        result.append(value)
        if depth == 0:
            break
    parsed = json.loads("".join(result))
    if not isinstance(parsed, dict):
        raise ValueError("Preset must contain an object")
    return parsed


def number(zone, name, fallback):
    value = float(zone.get(name, fallback))
    if not math.isfinite(value):
        raise ValueError(f"{name} must be finite")
    return value


def convert(source, directory, name=None, zones=None):
    text = read_source(source)
    preset = parse_preset(text)
    if not isinstance(preset, dict):
        raise ValueError("Preset must contain an object")
    entries = preset.get("zones", [])
    if not isinstance(entries, list) or not 1 <= len(entries) <= 256 or not all(isinstance(zone, dict) for zone in entries):
        raise ValueError("Preset must contain 1 to 256 zones")
    chosen = range(len(entries)) if zones is None else zones
    if not chosen or any(index < 0 or index >= len(entries) for index in chosen):
        raise ValueError("Zone index is outside the preset")
    directory = Path(directory)
    if directory.exists():
        raise ValueError("Choose a new output directory so existing instruments are never overwritten")
    directory.parent.mkdir(parents=True, exist_ok=True)
    name = re.sub(r"[^A-Za-z0-9 _-]", "_", name or Path(source).stem).strip()[:100] or "Instrument"
    with tempfile.TemporaryDirectory(prefix="musaic-bank-", dir=directory.parent) as temporary:
        temporary = Path(temporary)
        variants, provenance = [], []
        for index in chosen:
            zone = entries[index]
            rate = int(number(zone, "sampleRate", 44100))
            if not 1 <= rate <= 384000:
                raise ValueError("Invalid source sample rate")
            root = number(zone, "originalPitch", 6000) / 100 - number(zone, "coarseTune", 0) - number(zone, "fineTune", 0) / 100
            low, high = number(zone, "keyRangeLow", 0), number(zone, "keyRangeHigh", 127)
            if not 0 <= root <= 127 or not 0 <= low <= high <= 127:
                raise ValueError("Preset tuning or key range is outside MIDI 0 to 127")
            path = temporary / f"{name} - zone {index:03}.wav"
            if zone.get("sample"):
                decoded = base64.b64decode(zone["sample"], validate=True)
                if len(decoded) % 2:
                    raise ValueError("Raw PCM zone has an incomplete frame")
                if len(decoded) * 2 > MAX_BYTES - 44:
                    raise ValueError("Decoded PCM zone exceeds 64 MiB")
                # WebAudioFont divides signed 16-bit PCM by 65536, not 32768.
                payload = bytearray(len(decoded) * 2)
                for frame, value in enumerate(struct.iter_unpack("<h", decoded)):
                    struct.pack_into("<f", payload, frame * 4, value[0] / 65536)
                path.write_bytes(b"RIFF" + struct.pack("<I", 36 + len(payload)) + b"WAVEfmt " +
                    struct.pack("<IHHIIHH", 16, 3, 1, rate, rate * 4, 4, 32) + b"data" + struct.pack("<I", len(payload)) + payload)
                frame_count = len(decoded) // 2
            elif zone.get("file"):
                decoded = base64.b64decode(zone["file"], validate=True)
                path = path.with_suffix(".audio")
                path.write_bytes(decoded)
                frame_count = None  # The native importer validates after decoding.
            else:
                raise ValueError("Zone has no embedded sample or file data")
            if path.stat().st_size > MAX_BYTES or frame_count == 0:
                raise ValueError("Decoded zone is empty or exceeds 64 MiB")
            options = {"root_pitch": root, "default_gain": None}
            start, end = int(number(zone, "loopStart", 0)), int(number(zone, "loopEnd", 0))
            if 0 < start < end:
                if frame_count is not None and end > frame_count:
                    raise ValueError(f"Zone {index} loop ends beyond its decoded recording")
                options["sustain_loop"] = {"start_frame": start, "end_frame": end}
            variants.append({"path": path.name, "sample_rate": rate, "options": options, "pitch_zone": {"low": low, "high": high}})
            provenance.append({"zone": index, "source": source, "original_frames": frame_count,
                "root_pitch": root, "loop_frames": [start, end], "ahdsr": zone.get("ahdsr")})
        if sum(path.stat().st_size for path in temporary.iterdir()) > 256 * 1024 * 1024:
            raise ValueError("Bank exceeds 256 MiB")
        (temporary / "instrument.musaic-bank.json").write_text(
            json.dumps({"variants": variants}, indent=2) + "\n"
        )
        (temporary / "source.json").write_text(json.dumps(provenance, indent=2) + "\n")
        # Publish only the complete package after every zone has converted.
        temporary.rename(directory)
    return directory / "instrument.musaic-bank.json"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", help="Downloaded preset .js/JSON file or exact HTTPS preset URL")
    parser.add_argument("directory", help="New directory for the portable instrument bank")
    parser.add_argument("--name", help="Readable instrument name")
    parser.add_argument("--zones", help="Optional comma-separated zero-based source zone indexes")
    args = parser.parse_args()
    try:
        zones = [int(value) for value in args.zones.split(",")] if args.zones else None
        print(convert(args.source, args.directory, args.name, zones))
    except (ValueError, OSError) as error:
        parser.exit(1, f"Could not import instrument: {error}\n")


if __name__ == "__main__":
    main()
