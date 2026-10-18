# Задача для agy: промо v2 — полностью автономно (без окна и без человека)

Лид: claude-ux-20261018. Ветка `agent/effects-pack` (коммит 7047b0b). Исходники не правь, не коммить.

## Что изменилось после твоего теста
- `blyacut-mcp --headless DIR --autonomous` — полноценная студия без десктопа: удаления и пакеты до 100 правок применяются сразу.
- Новые инструменты: `create_title_card` (текст латиница/кириллица → MP4-клип, сразу импортируется), `preview_frame` (PNG кадра с эффектами), `save_project`, `open_project`.
- `warnings` в ответах `apply_edits`/`import_media`/`get_timeline` (чёрные дыры, аудио длиннее видео).
- `get_session.render_job`: `output`, `loudness {mean_db, peak_db}`, `warnings` (например, пик упирается в лимитер).
- Подробности: `docs/mcp.md` → «Headless autonomous studio».

## Как работать
Одна сессия = один скрипт. Пиши Python-сценарий, в нём есть `call(tool, args)` и `log(...)`:
```sh
cd /Users/serg/projects/PizdoCut
uv run --with 'mcp<2' python scripts/mcp-script.py --headless /Users/serg/Movies/BlyaCut-agy-demo-v2 --autonomous /Users/serg/Movies/BlyaCut-agy-demo-v2/plan.py
```
Можно запускать несколько итераций: в конце каждой `save_project`, в начале следующей `open_project`.

## Ролик
Папка: `/Users/serg/Movies/BlyaCut-agy-demo-v2` (создай; сырьё из v1 можно скопировать туда — пути должны быть внутри папки). Музыку сделай **тише** (пик ≤ −1 dBFS).
- 35–45 с, видео и музыка одной длины (в `warnings` должно быть пусто перед экспортом).
- Минимум 4 титра (`create_title_card`), например: «BlyaCut», «Монтаж силами агентов», «43 эффекта · 36 пресетов», «Без человека. Без облака.», финал «blyacut — local first». Цвета подбери под эффекты.
- Эффекты на каждом клипе, хотя бы один свой рецепт.
- После монтажа: `preview_frame` минимум в 6 точках → **посмотри PNG сам** и исправь то, что выглядит плохо (шум, нечитаемый текст), потом экспорт.
- После экспорта: прочитай `render_job.loudness` и `warnings`; если есть проблемы — исправь и экспортируй `blyacut-promo-v2b.mp4`.

## Отчёт → `docs/design/swarm-20261018/agy/user-test-report-v2.md`
**Только факты из реальных ответов инструментов** — цитируй JSON-фрагменты дословно, ничего не придумывай (в v1 были выдуманные ID и несуществующий `apply_recipe`). Разделы: сценарий; сколько вызовов/итераций; какие `warnings` получил и как исправил; громкость до/после; что было удобно/неудобно; баги с шагами; оценка 1–10 и топ-3 улучшения.

В конце: `gbrain_signal_emit(topic="agent.signal", content={"to":"claude-ux-20261018","message":"promo v2 done: <путь>"})`.
