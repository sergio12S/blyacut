use blyacut_core::{
    effects::{EffectRecipe, EffectStep},
    *,
};
#[test]
fn recipes_are_strict_bounded_and_project_embeds_a_snapshot() {
    let mut e = Editor::default();
    e.import(Asset {
        id: "v".into(),
        name: "v.mp4".into(),
        path: "/tmp/v.mp4".into(),
        duration_ms: 1000,
        width: 320,
        height: 180,
        has_audio: true,
    })
    .unwrap();
    e.execute(Edit::Append {
        asset_id: "v".into(),
        clip_id: "c".into(),
        track_id: e.project.tracks[0].id.clone(),
    })
    .unwrap();
    let mut recipe = EffectRecipe {
        version: 1,
        id: "custom-look".into(),
        name: "Custom look".into(),
        steps: vec![
            EffectStep::Saturation { gain_milli: 0 },
            EffectStep::Contrast { gain_milli: 1200 },
        ],
    };
    e.execute(Edit::SetEffects {
        clip_id: "c".into(),
        effects: vec![recipe.clone()],
    })
    .unwrap();
    let before = e.project.clone();
    recipe.steps = vec![EffectStep::Hue { degrees: 90 }];
    assert_eq!(e.project, before);
    assert!(serde_json::from_str::<EffectRecipe>(r#"{"version":1,"id":"bad","name":"Bad","steps":[{"effect":"raw_ffmpeg","filter":"movie=/etc/passwd"}]}"#).is_err());
    for recipe in [
        EffectRecipe {
            steps: vec![EffectStep::LowPass { frequency_hz: 0 }],
            ..recipe.clone()
        },
        EffectRecipe {
            id: "../../escape".into(),
            ..recipe.clone()
        },
        EffectRecipe {
            version: 99,
            ..recipe.clone()
        },
    ] {
        assert!(e
            .execute(Edit::SetEffects {
                clip_id: "c".into(),
                effects: vec![recipe]
            })
            .is_err());
        assert_eq!(e.project, before);
    }
    e.undo();
    assert!(e.project.tracks[0].clips[0].effects.is_empty());
    e.redo();
    assert_eq!(e.project, before);
}
