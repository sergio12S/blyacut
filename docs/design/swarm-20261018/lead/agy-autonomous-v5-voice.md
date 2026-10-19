# Задача для agy: ролик v5 — голос, субтитры, приглушение, анимация (автономно)

Лид: claude-ux-20261018. Ветка `agent/ducking-keyframes` (коммит c3b562d). Исходники не правь, не коммить.

## Новое (см. `docs/mcp.md`)
- **Субтитры делаешь ты сам, BlyaCut их только раскладывает.** `extract_audio {clip_id, output_path .wav}` → 16 кГц моно WAV + `timeline_offset_ms`. Расшифруй своими средствами (свой speech-to-text, если он у тебя есть). Затем `add_captions {srt | captions:[{start_ms,end_ms,text}], output_dir}` — дорожка «Subtitles».
- `set_ducking {clip_id музыки, duck_db}` — музыка проседает под голос.
- `set_keyframes {clip_id наложения, keyframes:[{at_ms, transform}]}` — выезды/наезды/проявления (x/y можно -500..1500).
- `set_speed {clip_id, speed_milli, reverse}`, `freeze_frame {clip_id, at_ms, duration_ms, output_path .png}`.

## Ролик
Папка `~/Movies/BlyaCut-agy-demo-v5` (создай; сырьё скопируй из v2–v4). 30–45 с, 1920×1080.
1. **Голос за кадром:** сгенерируй русскую озвучку системной командой macOS, например
   `say -v Milena -o voice.aiff "Это BlyaCut. Агент сам монтирует видео, ..."` и сконвертируй в WAV через ffmpeg. 3–5 фраз. Положи на звуковую дорожку.
2. **Субтитры по-честному:** `extract_audio` клипа с голосом → расшифруй **своим** распознаванием → `add_captions`. Если у тебя нет speech-to-text — **так и напиши в отчёте**, и только тогда используй текст сценария (тайминги всё равно подбирай по WAV, например по паузам через ffmpeg silencedetect).
3. **Музыка** на второй звуковой дорожке с `set_ducking` 12–18 дБ.
4. Минимум 1 анимированное наложение (`set_keyframes`: выезд или наезд), 1 ускорение или замедление (`set_speed`), 1 `freeze_frame`.
5. `preview_frame` в 5+ точках, `warnings` пустые, пик ≤ −1 dBFS.

## Отчёт → `docs/design/swarm-20261018/agy/user-test-report-v5.md`
Только дословные факты. Обязательно: чем расшифровывал (или что не смог), насколько субтитры совпали со сказанным, как работает приглушение на слух/по замеру, что неудобно, баги, оценка 1–10, топ-3 улучшения.

В конце: `gbrain_signal_emit(topic="agent.signal", content={"to":"claude-ux-20261018","message":"v5 done: <путь>"})`.
