#!/usr/bin/env python3
"""Generate synthetic example assets. Existing files are preserved."""
import pathlib
import shutil
import subprocess

root=pathlib.Path(__file__).resolve().parents[1]
ffmpeg=shutil.which('ffmpeg')
if not ffmpeg:
    ffmpeg=next((p for p in ['/opt/homebrew/bin/ffmpeg','/usr/local/bin/ffmpeg'] if pathlib.Path(p).exists()),None)
if not ffmpeg:
    raise SystemExit('Install FFmpeg first.')
for name,inputs,codecs in [
    ('synthetic.mp4',['-f','lavfi','-i','testsrc2=size=640x360:rate=30','-f','lavfi','-i','sine=frequency=440:sample_rate=48000'],['-c:v','libx264','-pix_fmt','yuv420p','-c:a','aac']),
    ('blue.mp4',['-f','lavfi','-i','color=c=blue:s=640x360:r=30','-f','lavfi','-i','sine=frequency=220:sample_rate=48000'],['-c:v','libx264','-pix_fmt','yuv420p','-c:a','aac']),
    ('soundtrack.wav',['-f','lavfi','-i','sine=frequency=880:sample_rate=48000'],['-c:a','pcm_s16le']),
]:
    path=root/'examples'/name
    if path.exists():
        print(f'Preserved existing {name}')
        continue
    subprocess.run([ffmpeg,'-v','error','-n',*inputs,'-t','6',*codecs,str(path)],check=True)
    print(f'Generated {name}')
print('Open examples/first-cut.blyacut, or follow docs/mcp.md for the two-video/audio demo.')
