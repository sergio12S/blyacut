#!/usr/bin/env python3
"""Generate an inventory for review; this is not a license compliance verdict."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(
    ['cargo', 'metadata', '--format-version', '1', '--locked'], cwd=root
))
lines = ['# Dependency license inventory', '', 'Generated from Cargo.lock and installed npm packages. Review before distribution.', '', '## Rust', '', '| Package | Version | Declared license |', '|---|---|---|']
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    lines.append(f"| {package['name']} | {package['version']} | {(package.get('license') or 'REVIEW: no SPDX declaration').replace('|', '/')} |")
lines += ['', '## npm', '', '| Package | Version | Declared license |', '|---|---|---|']
for package_path in sorted((root / 'node_modules').glob('*/package.json')) + sorted((root / 'node_modules').glob('@*/*/package.json')):
    package = json.loads(package_path.read_text())
    lines.append(f"| {package['name']} | {package['version']} | {package.get('license', 'REVIEW: no declaration')} |")
(root / 'docs' / 'dependency-licenses.md').write_text('\n'.join(lines) + '\n')
