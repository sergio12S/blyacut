use blyacut_core::{effects::*, *};
use blyacut_engine::{studio::*, *};
use std::process::Command;

fn pixel(png: &std::path::Path, x: f64, y: f64) -> [u8; 3] {
    let out = Command::new(binary("ffmpeg"))
        .args(["-v", "error", "-i"])
        .arg(png)
        .args([
            "-vf",
            &format!("crop=1:1:iw*{x}:ih*{y},format=rgb24"),
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    [out.stdout[0], out.stdout[1], out.stdout[2]]
}

#[test]
fn preview_title_and_loudness_work_for_agents() {
    let dir = std::env::temp_dir().join(format!("blyacut-studio-{}", id()));
    std::fs::create_dir(&dir).unwrap();
    let mut e = Editor::default();
    e.project.settings.width = 640;
    e.project.settings.height = 360;
    let style = TitleStyle {
        text_rgb: [255, 255, 255],
        background_rgb: [10, 20, 200],
    };
    let title = dir.join("title.mp4");
    title_card(&e.project, "BlyaCut\nмонтаж агентами", 2000, &style, &title).unwrap();
    assert!(
        title_card(&e.project, "x", 2000, &style, &title).is_err(),
        "never overwrites"
    );
    let asset = probe(&title).unwrap();
    assert!(asset.duration_ms.abs_diff(2000) < 100 && asset.width == 640 && asset.has_audio);
    let aid = asset.id.clone();
    e.import(asset).unwrap();
    let vt = e.project.tracks[0].id.clone();
    e.batch(
        &[Edit::Append {
            asset_id: aid,
            clip_id: "t".into(),
            track_id: vt,
        }],
        e.revision,
    )
    .unwrap();

    // Plain frame: blue background, white glyphs somewhere on the centre row band.
    let plain = dir.join("plain.png");
    preview_frame(&e.project, 1000, &plain, 320).unwrap();
    let bg = pixel(&plain, 0.02, 0.02);
    assert!(bg[2] > 150 && bg[0] < 60, "background {bg:?}");
    let any_white = (5..95).any(|i| {
        pixel(&plain, i as f64 / 100.0, 0.4)
            .iter()
            .all(|c| *c > 200)
    });
    assert!(any_white, "no title glyphs rendered");

    // Effects are applied exactly as in export: invert turns the blue background yellow.
    let invert = EffectRecipe {
        version: 1,
        id: "inv".into(),
        name: "inv".into(),
        steps: vec![EffectStep::Invert {}],
    };
    e.execute(Edit::SetEffects {
        clip_id: "t".into(),
        effects: vec![invert],
    })
    .unwrap();
    let fx = dir.join("fx.png");
    preview_frame(&e.project, 1000, &fx, 320).unwrap();
    let p = pixel(&fx, 0.02, 0.02);
    assert!(p[0] > 200 && p[1] > 180 && p[2] < 80, "not inverted {p:?}");
    // Past the end is black.
    let black = dir.join("black.png");
    preview_frame(&e.project, 5000, &black, 320).unwrap();
    assert!(pixel(&black, 0.5, 0.5).iter().all(|c| *c < 10));

    let level = loudness(&title).unwrap();
    assert!(
        level.peak_db < -80.0,
        "title audio should be silent: {level:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
