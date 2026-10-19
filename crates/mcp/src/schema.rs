use serde_json::{json, Value};
fn object(fields: Value, required: Vec<&str>) -> Value {
    json!({"type":"object","properties":fields,"required":required,"additionalProperties":false})
}
fn edit(operation: &str, fields: Value, required: Vec<&str>) -> Value {
    let mut fields = fields.as_object().unwrap().clone();
    fields.insert(
        "operation".into(),
        json!({"const":operation,"type":"string"}),
    );
    let mut required = required;
    required.push("operation");
    object(Value::Object(fields), required)
}
pub fn tools() -> Vec<Value> {
    let string = json!({"type":"string","minLength":1});
    let time = json!({"type":"integer","minimum":0,"maximum":604800000});
    let revision = json!({"type":"integer","minimum":0});
    // Overlay placement; x/y may go off-frame (-500..1500) for slide-ins.
    let transform = json!({"type":"object","properties":{"x_milli":{"type":"integer","minimum":-500,"maximum":1500},"y_milli":{"type":"integer","minimum":-500,"maximum":1500},"scale_milli":{"type":"integer","minimum":10,"maximum":4000},"opacity_milli":{"type":"integer","minimum":0,"maximum":1000}},"required":["x_milli","y_milli","scale_milli","opacity_milli"],"additionalProperties":false});
    let edits = json!({"type":"array","minItems":1,"maxItems":100,"items":{"oneOf":[
     edit("set_effects",json!({"clip_id":string,"effects":{"type":"array","maxItems":8,"items":blyacut_core::effects::recipe_schema()}}),vec!["clip_id","effects"]),
     edit("set_settings",json!({"settings":{"type":"object","properties":{"width":{"type":"integer","minimum":2,"maximum":7680},"height":{"type":"integer","minimum":2,"maximum":4320},"fps_num":{"type":"integer","minimum":1,"maximum":240000},"fps_den":{"type":"integer","minimum":1,"maximum":1000},"fill":{"type":"boolean"}},"required":["width","height","fps_num","fps_den"],"additionalProperties":false}}),vec!["settings"]),
     edit("set_transition",json!({"clip_id":string,"transition":{"oneOf":[{"type":"null"},{"type":"object","properties":{"kind":{"type":"string","enum":blyacut_core::TransitionKind::ALL.iter().map(|k| k.name()).collect::<Vec<_>>()},"duration_ms":{"type":"integer","minimum":100,"maximum":5000}},"required":["kind","duration_ms"],"additionalProperties":false}]},"make_room":{"type":"boolean"}}),vec!["clip_id","transition"]),
     edit("set_ducking",json!({"clip_id":string,"duck_db":{"type":"integer","minimum":0,"maximum":30}}),vec!["clip_id","duck_db"]),
     edit("set_speed",json!({"clip_id":string,"speed_milli":{"type":"integer","minimum":250,"maximum":4000},"reverse":{"type":"boolean"}}),vec!["clip_id","speed_milli"]),
     edit("set_transform",json!({"clip_id":string,"transform":{"oneOf":[{"type":"null"},transform.clone()]}}),vec!["clip_id","transform"]),
     edit("set_keyframes",json!({"clip_id":string,"keyframes":{"type":"array","maxItems":32,"items":{"type":"object","properties":{"at_ms":time,"transform":transform.clone()},"required":["at_ms","transform"],"additionalProperties":false}}}),vec!["clip_id","keyframes"]),
     edit("add_track",json!({"track_id":string,"name":string,"kind":{"type":"string","enum":["video","audio"]}}),vec!["track_id","name","kind"]),
     edit("append",json!({"asset_id":string,"clip_id":string,"track_id":string}),vec!["asset_id","clip_id","track_id"]),
     edit("split_clip",json!({"clip_id":string,"position_ms":time,"new_clip_id":string}),vec!["clip_id","position_ms","new_clip_id"]),
     edit("trim_clip",json!({"clip_id":string,"start_ms":time,"end_ms":time}),vec!["clip_id","start_ms","end_ms"]),
     edit("delete_clip",json!({"clip_id":string}),vec!["clip_id"]),
     edit("reorder_clip",json!({"clip_id":string,"index":{"type":"integer","minimum":0}}),vec!["clip_id","index"]),
     edit("move_clip",json!({"clip_id":string,"track_id":string,"position_ms":time}),vec!["clip_id","track_id","position_ms"]),
     edit("set_audio",json!({"clip_id":string,"gain_milli":{"type":"integer","minimum":0,"maximum":1000},"fade_in_ms":time,"fade_out_ms":time}),vec!["clip_id","gain_milli","fade_in_ms","fade_out_ms"])
    ]}});
    let batch = object(
        json!({"expected_revision":revision,"edits":edits}),
        vec!["expected_revision", "edits"],
    );
    let mut tools = vec![
        json!({"name":"list_effect_recipes","description":"Reload validated JSON recipes from the authorized folder's effects directory. Returns folder recipes, per-file warnings and `builtin` presets shipped with BlyaCut (apply any of them with set_effects). No app restart required.","inputSchema":object(json!({}),vec![]),"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"create_effect_recipe","description":"Create a named visual/audio effect recipe under authorized-folder/effects. Compose supported primitives; definitions are data, never scripts or raw FFmpeg strings. Existing files are not overwritten. Apply the returned recipe with set_effects; it is embedded in the project.","inputSchema":object(json!({"recipe":blyacut_core::effects::recipe_schema()}),vec!["recipe"]),"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
    ];
    for (name,description,schema,read_only) in [
  ("get_project","Read the live project and monotonic revision. Use its IDs for subsequent edits.",object(json!({}),vec![]),true),
  ("get_timeline","Inspect live tracks, source boundaries, timeline duration and `warnings` (black gaps, audio running past the video).",object(json!({}),vec![]),true),
  ("get_session","Read authorized folder, mode (review|autonomous), current export job (status, output, loudness, warnings) and pending human-review proposal.",object(json!({}),vec![]),true),
  ("import_media","Import existing video, audio or a still image (PNG/JPEG/WebP; usable for up to 1 h) from the authorized folder. Returns the updated project.",object(json!({"path":string}),vec!["path"]),false),
  ("apply_edits","Atomically apply up to 20 edits with one Undo. Animation: set_keyframes {clip_id of an overlay clip, keyframes:[{at_ms (from the clip start, increasing), transform}]} moves/scales/fades it linearly between keys (e.g. x_milli -200 -> 500 over 600 ms slides in from the left); [] removes it. Ducking: set_ducking {clip_id of an AUDIO-track clip such as music, duck_db 3..30 or 0 = off} lowers it by about duck_db while voice plays (main-video sound and non-ducking audio clips). Speed: set_speed {clip_id, speed_milli 250..4000 (1000 = normal; timeline length = source span ×1000/speed), reverse}; trim still uses SOURCE ms. Format: set_settings {width,height (even),fps_num,fps_den,fill} — e.g. 1080x1920 fill:true for vertical from landscape footage. Transitions: set_transition on a main-track clip (not the first) blends from the clip before it, centred on the cut; it needs duration/2 ms of source media after the outgoing clip end and before the incoming clip start; pass make_room:true to trim those edges automatically (stills always have handles). In review mode removals need propose_edits; in autonomous mode up to 100 edits incl. delete_clip apply directly. The result includes `warnings`. Times are integer milliseconds; trim uses source time, move/split use timeline time. gain_milli is 0–1000. Audio tracks do not ripple; video trim/delete/reorder ripple only their track.",batch.clone(),false),
  ("propose_edits","Preview up to 100 edits without changing the timeline. User approves or rejects in desktop UI. Required for removals and batches over 20. Approval fails after intervening edits.",batch,false),
  ("undo","Undo one entire action/batch at the expected revision.",object(json!({"expected_revision":revision}),vec!["expected_revision"]),false),
  ("redo","Redo one action/batch at the expected revision.",object(json!({"expected_revision":revision}),vec!["expected_revision"]),false),
  ("export_video","Start asynchronous H.264/AAC MP4 export to a NEW absolute path inside the authorized folder. Poll get_session for completion. One video track plus independent audio tracks supported.",object(json!({"expected_revision":revision,"output_path":string}),vec!["expected_revision","output_path"]),false),
  ("cancel_export","Request cancellation of current export; original media remains unchanged.",object(json!({}),vec![]),false),
  ("preview_frame","Render what the export will show at one timeline instant (effects included) to a NEW absolute .png inside the authorized folder, then look at it. Default width 960.",object(json!({"time_ms":time,"output_path":string,"width":{"type":"integer","minimum":64,"maximum":3840}}),vec!["time_ms","output_path"]),false),
  ("create_title_card","Render centered text (1–200 chars, up to 4 lines with \\n, Latin/Cyrillic) as a NEW .mp4 title clip in the project size inside the authorized folder and import it (import:false to skip). Colors are #RRGGBB. Returns asset_id; append it like any clip and add effects such as builtin-fade-in.",object(json!({"text":string,"output_path":string,"duration_ms":{"type":"integer","minimum":500,"maximum":60000},"text_color":string,"background_color":string,"import":{"type":"boolean"}}),vec!["text","output_path","duration_ms"]),false),
  ("create_text_overlay","Render text (1–200 chars, up to 4 lines, Latin/Cyrillic) on a TRANSPARENT full-frame NEW .png inside the authorized folder and import it as a still. style: box (default, dark plate — legible on any footage) | outline | shadow | none. position: top|center|bottom (vertical frames keep bottom text above the Shorts/Reels/TikTok UI). Long lines wrap automatically in narrow frames. Pass place {track_id of an overlay video track (add_track kind video = V2+), position_ms, duration_ms} to put it on the timeline in the same call; the result has clip_id. Without place, append + move_clip + trim_clip it yourself (a still is 1 h long until trimmed).",object(json!({"text":string,"output_path":string,"position":{"type":"string","enum":["top","center","bottom"]},"text_color":string,"style":{"type":"string","enum":["box","outline","shadow","none"]},"import":{"type":"boolean"},"place":{"type":"object","properties":{"track_id":string,"position_ms":time,"duration_ms":{"type":"integer","minimum":1,"maximum":3600000}},"required":["track_id","position_ms","duration_ms"],"additionalProperties":false}}),vec!["text","output_path"]),false),
  ("freeze_frame","Hold the main-track frame at timeline at_ms for duration_ms (100–60000): writes the exact source frame to a NEW .png inside the authorized folder, splits the clip there and inserts the still with the clip's effects; later clips ripple. Returns clip_id of the still.",object(json!({"clip_id":string,"at_ms":time,"duration_ms":{"type":"integer","minimum":100,"maximum":60000},"output_path":string}),vec!["clip_id","at_ms","duration_ms","output_path"]),false),
  ("save_project","Save the live project as .blyacut inside the authorized folder (overwrites that project file). Media is referenced, not copied.",object(json!({"path":string}),vec!["path"]),false),
  ("open_project","Replace the live project with a .blyacut file from the authorized folder; all its media must also be inside the folder.",object(json!({"path":string}),vec!["path"]),false),
 ] {tools.push(json!({"name":name,"description":description,"inputSchema":schema,"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"openWorldHint":false}}));}
    tools
}
