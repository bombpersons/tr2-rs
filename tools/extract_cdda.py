#!/usr/bin/env python3
"""Bake original CDDA PCM tracks into standalone, ordinary WAV assets."""
import argparse
import hashlib
import json
import wave
from pathlib import Path

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom', type=Path, default=root / 'rom', help="folder with the disc's per-track .bin files")
parser.add_argument('--output', type=Path, default=root / 'game/assets/music')
args = parser.parse_args()
output = args.output
output.mkdir(parents=True, exist_ok=True)
manifest = []
for source in sorted(args.rom.glob('*Track *.bin')):
    number = int(source.stem.rsplit('Track ', 1)[1].rstrip(')'))
    if number == 1:
        continue
    pcm = source.read_bytes()
    assert len(pcm) % 2352 == 0
    destination = output / f'{number:02}.wav'
    with wave.open(str(destination), 'wb') as sound:
        sound.setnchannels(2)
        sound.setsampwidth(2)
        sound.setframerate(44100)
        sound.writeframes(pcm)
    manifest.append(dict(track=number,path=destination.name,seconds=len(pcm)/176400,
                         source_sha256=hashlib.sha256(pcm).hexdigest()))
(output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'Extracted {len(manifest)} CDDA tracks into standalone WAV files')
