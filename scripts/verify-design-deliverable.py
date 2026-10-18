#!/usr/bin/env python3
"""Concrete artifact contract for the GBRAIN design swarm."""
import json
import pathlib
import sys
role=sys.argv[1]
root=pathlib.Path(__file__).resolve().parents[1]/'docs/design/swarm-20261018'/role
brief=(root/'brief.md').read_text()
assert len(brief)>1800,'Design brief must contain concrete flows, states, constraints and handoff.'
html=(root/'prototype.html').read_text()
assert len(html)>5000 and '<html' in html.lower() and '<style' in html.lower(),'Provide a standalone inspectable HTML/CSS prototype.'
assert 'timeline' in html.lower() and 'agent' in html.lower(),'Prototype must address timeline and agent workflow.'
if role=='grok':
    tokens=json.loads((root/'tokens.json').read_text())
    for key in ('colors','spacing','typography','radii'):
        assert isinstance(tokens[key],dict) and tokens[key],f'Missing token group {key}'
if role=='agy':
    contract=json.loads((root/'agent-contract.json').read_text())
    for key in ('capabilities','permissions','jobs','effects','recovery'):
        assert key in contract,f'Missing agent experience contract {key}'
print(f'{role}: design deliverables satisfy artifact contract')
