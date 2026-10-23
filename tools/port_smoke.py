#!/usr/bin/env python3
"""Capture real offscreen Bevy frames over stdio MCP (never starts cargo).

Run after `cargo build -p tomb-raider-ii --features dev-mcp`.
All artifacts and exact MCP operations are retained under validation/port-smoke.
The original disc is not opened. Reference values came from original-evidence.md.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import json
import math
import os
from pathlib import Path
import queue
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[1]


def serializable(value):
    if isinstance(value, dict):
        return {key: (f'<image {len(item)} base64 characters>' if key == 'data' and value.get('type') == 'image' else serializable(item)) for key, item in value.items()}
    if isinstance(value, list):
        return [serializable(item) for item in value]
    return value


class Client:
    def __init__(self, binary: Path, out: Path, level: str | None, asset_root: Path | None = None, extra_args: list[str] | None = None):
        self.out = out
        self.out.mkdir(parents=True, exist_ok=True)
        self.log = (out / 'game.log').open('w')
        self.transcript = (out / 'mcp.jsonl').open('w')
        self.identifier = 0
        self.messages = queue.Queue()
        args = [str(binary), '--mcp', '--headless', '--software']
        if level is not None:
            args += ['--level', level]
        args += extra_args or []
        env = os.environ.copy()
        env.pop('DISPLAY', None)
        env.pop('WAYLAND_DISPLAY', None)
        env['BEVY_ASSET_ROOT'] = str(asset_root or ROOT / 'game')
        # CI has no speaker device. ALSA's null output avoids PulseAudio/JACK
        # connection retries while still exercising the game's audio plugin.
        alsa = out / 'alsa-null.conf'
        alsa.write_text('pcm.!default { type null }\nctl.!default { type hw card 0 }\n')
        env['ALSA_CONFIG_PATH'] = str(alsa)
        self.process = subprocess.Popen(args, cwd=ROOT / 'game', env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log, text=True, bufsize=1)
        threading.Thread(target=self._read, daemon=True).start()
        try:
            self.request('initialize', {'protocolVersion': '2025-06-18', 'capabilities': {}, 'clientInfo': {'name': 'tr2-port-smoke', 'version': '1'}})
            self.send({'jsonrpc': '2.0', 'method': 'notifications/initialized'})
        except Exception:
            self.close()
            raise

    def _read(self):
        for line in self.process.stdout:
            self.messages.put(line)
        self.messages.put(None)

    def send(self, message):
        self.transcript.write(json.dumps({'request': message}) + '\n')
        self.transcript.flush()
        self.process.stdin.write(json.dumps(message) + '\n')
        self.process.stdin.flush()

    def request(self, method, params):
        self.identifier += 1
        self.send({'jsonrpc': '2.0', 'id': self.identifier, 'method': method, 'params': params})
        deadline = time.monotonic() + 120
        while True:
            try:
                line = self.messages.get(timeout=max(0.01, deadline - time.monotonic()))
            except queue.Empty:
                raise RuntimeError(f'MCP timeout; inspect {self.out / "game.log"}')
            if line is None:
                raise RuntimeError(f'Game exited ({self.process.poll()}); inspect {self.out / "game.log"}')
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                raise RuntimeError(f'Non-JSON data on MCP stdout: {line[:200]}')
            self.transcript.write(json.dumps({'response': serializable(message)}) + '\n')
            self.transcript.flush()
            if message.get('id') == self.identifier:
                if 'error' in message:
                    raise RuntimeError(message['error'])
                return message['result']

    def call(self, name, arguments=None):
        result = self.request('tools/call', {'name': name, 'arguments': arguments or {}})
        if result.get('isError'):
            raise RuntimeError(f'{name}: {result}')
        return result

    def query(self, name='Player', components=None, resources=None):
        args = {'name': name}
        if components is not None:
            args['components'] = components
        if resources is not None:
            args['resources'] = resources
        return self.call('game_query', args).get('structuredContent', {})

    def wait_loaded(self):
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            state = self.query(components=['Player', 'Transform', 'AnimationState', 'Motion'])
            if state.get('entities'):
                # Texture loading and render extraction can trail entity creation.
                time.sleep(0.5)
                return state
            time.sleep(0.1)
        raise RuntimeError(f'Player failed to load; inspect {self.out / "game.log"}')

    def edit(self, name, component, fields):
        return self.call('game_set_component', {'name': name, 'component': component, 'fields': fields}).get('structuredContent', {})

    def capture(self, name, min_colors=64):
        result = self.call('game_screenshot', {'target': 'screen'})
        block = next(item for item in result['content'] if item['type'] == 'image')
        data = base64.b64decode(block['data'])
        path = self.out / f'{name}.png'
        path.write_bytes(data)
        record = {'path': str(path.relative_to(ROOT)), 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
        try:
            from PIL import Image
            with Image.open(path) as im:
                record.update(width=im.width, height=im.height, unique_colors=len(im.convert('RGB').getcolors(im.width * im.height)))
                if record['unique_colors'] < min_colors:
                    raise RuntimeError(f'{path}: only {record["unique_colors"]} colors; render may be blank')
        except ImportError:
            pass
        return record

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.log.close()
        self.transcript.close()


def convert(v):
    return [v[0] / 1024, -v[1] / 1024, v[2] / 1024]


REFERENCES = {
    'home': dict(level='assault', checkpoint='.psx/home_start/state.bin', screenshot='.psx/home_start/screen.png', position=[19968, 2560, 95744], yaw=32768, room=16, animation=103, absolute_animation_frame=1649, camera=[20817, 1967, 97024], target=[19968, 1976, 95744]),
    'wall': dict(level='wall', checkpoint='.psx/wall_game/state.bin', screenshot='.psx/wall_game/screen.png', position=[64176, 5376, 26090], yaw=0, room=35, animation=103, absolute_animation_frame=1653, camera=[63752, 4787, 25846], target=[64176, 4795, 26090]),
}


def align_reference(client, reference):
    yaw = reference['yaw'] * math.tau / 65536
    feet = convert(reference['position'])
    level = json.loads((ROOT / f'game/assets/imported/levels/{reference["level"]}/level.json').read_text())
    clip_start = level['animations'][reference['animation']]['frame_start']
    edits = {}
    edits['player'] = client.edit('Player', 'Player', {'position': feet, 'yaw': yaw, 'room': reference['room'], 'health': 1000})
    edits['transform'] = client.edit('Player', 'Transform', {'translation': feet, 'rotation': [0, math.sin(yaw / 2), 0, math.cos(yaw / 2)]})
    edits['animation'] = client.edit('Player', 'AnimationState', {'animation': reference['animation'], 'frame': reference['absolute_animation_frame'] - clip_start})
    edits['camera'] = client.edit('ChaseCamera', 'CameraRig', {'manual': True, 'position': convert(reference['camera']), 'target': convert(reference['target'])})
    return edits


def run(args):
    args.binary = args.binary.resolve()
    args.out = args.out.resolve()
    report = {'binary': str(args.binary), 'binary_sha256': hashlib.sha256(args.binary.read_bytes()).hexdigest(), 'started_utc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()), 'cases': {}, 'status': 'running'}
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        for case in args.cases.split(','):
            if case not in ['home', 'wall', 'title']:
                raise ValueError(f'Unknown case {case}')
            reference = REFERENCES.get(case)
            client = Client(args.binary, args.out / case, reference['level'] if reference else None)
            evidence = report['cases'][case] = {}
            try:
                evidence['loaded_state'] = client.wait_loaded()
                evidence['status'] = client.call('game_status').get('structuredContent')
                evidence['initial'] = client.capture('initial')
                if reference:
                    evidence['original_reference'] = reference
                    evidence['edits'] = align_reference(client, reference)
                    evidence['aligned'] = client.capture('aligned-original')
                    evidence['aligned_state'] = client.query(components=['Player', 'Transform', 'AnimationState'])
                    evidence['camera_state'] = client.query('ChaseCamera', ['CameraRig', 'Transform'])
                    if case == 'home':
                        client.call('game_step', {'frames': 1, 'buttons': ['start']})
                        evidence['pause_menu'] = client.capture('pause-menu')
                        evidence['pause_state'] = client.query(resources=['Session'])
                        client.call('game_step', {'frames': 1, 'buttons': ['triangle']})
                        client.call('game_step', {'frames': 1, 'buttons': ['select']})
                        evidence['inventory_menu'] = client.capture('inventory-menu')
                        evidence['inventory_state'] = client.query(resources=['Session'])
                else:
                    evidence['title_state'] = client.query(resources=['Session'])
            finally:
                client.close()
            logs = (args.out / case / 'game.log').read_text()
            failures = [line for line in logs.splitlines() if any(word in line for word in ['Shader compilation error', 'Error matching ShaderStages', 'panicked at', 'Failed to load asset'])]
            evidence['render_errors'] = failures
            if failures:
                raise RuntimeError(f'{case}: render/asset errors in game.log')
        report['status'] = 'complete'
    except Exception as error:
        report['status'] = 'failed'
        report['error'] = str(error)
        raise
    finally:
        (args.out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/tomb-raider-ii')
    parser.add_argument('--out', type=Path, default=ROOT / 'validation/port-smoke')
    parser.add_argument('--cases', default='home,wall,title')
    run(parser.parse_args())
