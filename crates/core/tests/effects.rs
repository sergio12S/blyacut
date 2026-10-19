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
        still: false,
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

#[test]
fn every_spec_entry_round_trips_and_builtins_validate() {
    use blyacut_core::effects::{builtin_recipes, recipe_schema, SPEC};
    for (name, video, ranges) in SPEC {
        let mut step = serde_json::json!({ "effect": name });
        for (param, min, max) in ranges.iter() {
            step[*param] = serde_json::json!((min + max) / 2);
        }
        let parsed: EffectStep = serde_json::from_value(step.clone())
            .unwrap_or_else(|e| panic!("{name} does not deserialize: {e}"));
        parsed.validate().unwrap();
        assert_eq!(parsed.is_video(), *video, "{name}");
        assert_eq!(serde_json::to_value(&parsed).unwrap(), step);
        for (param, _, max) in ranges.iter() {
            let mut out = step.clone();
            out[*param] = serde_json::json!(max + 1);
            if let Ok(s) = serde_json::from_value::<EffectStep>(out) {
                assert!(s.validate().is_err(), "{name}.{param} accepts {}", max + 1);
            }
        }
    }
    let schema = recipe_schema();
    assert_eq!(
        schema["properties"]["steps"]["items"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        SPEC.len()
    );
    let builtins = builtin_recipes();
    assert!(builtins.len() >= 30);
    let mut ids = std::collections::HashSet::new();
    for recipe in &builtins {
        recipe.validate().unwrap();
        assert!(recipe.id.starts_with("builtin-") && ids.insert(recipe.id.clone()));
        let video = recipe.steps.iter().filter(|s| s.is_video()).count();
        assert!(
            video == 0 || video == recipe.steps.len(),
            "{} mixes kinds",
            recipe.id
        );
    }
}
