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
    let edits = json!({"type":"array","minItems":1,"maxItems":100,"items":{"oneOf":[
     edit("set_effects",json!({"clip_id":string,"effects":{"type":"array","maxItems":4,"items":blyacut_core::effects::recipe_schema()}}),vec!["clip_id","effects"]),
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
        json!({"name":"list_effect_recipes","description":"Reload validated JSON recipes from the authorized folder's effects directory. Returns recipes and per-file warnings. No app restart required.","inputSchema":object(json!({}),vec![]),"annotations":{"readOnlyHint":true,"openWorldHint":false}}),
        json!({"name":"create_effect_recipe","description":"Create a named visual/audio effect recipe under authorized-folder/effects. Compose supported primitives; definitions are data, never scripts or raw FFmpeg strings. Existing files are not overwritten. Apply the returned recipe with set_effects; it is embedded in the project.","inputSchema":object(json!({"recipe":blyacut_core::effects::recipe_schema()}),vec!["recipe"]),"annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":false}}),
    ];
    for (name,description,schema,read_only) in [
  ("get_project","Read the live project and monotonic revision. Use its IDs for subsequent edits.",object(json!({}),vec![]),true),
  ("get_timeline","Inspect live tracks, source boundaries and timeline duration.",object(json!({}),vec![]),true),
  ("get_session","Read authorized folder, current export job and pending human-review proposal.",object(json!({}),vec![]),true),
  ("import_media","Import existing video or audio from the authorized folder. Returns the updated project.",object(json!({"path":string}),vec!["path"]),false),
  ("apply_edits","Atomically apply up to 20 edits with one Undo. Removal needs propose_edits. Times are integer milliseconds; trim uses source time, move/split use timeline time. gain_milli is 0–1000. Audio tracks do not ripple; video trim/delete/reorder ripple only their track.",batch.clone(),false),
  ("propose_edits","Preview up to 100 edits without changing the timeline. User approves or rejects in desktop UI. Required for removals and batches over 20. Approval fails after intervening edits.",batch,false),
  ("undo","Undo one entire action/batch at the expected revision.",object(json!({"expected_revision":revision}),vec!["expected_revision"]),false),
  ("redo","Redo one action/batch at the expected revision.",object(json!({"expected_revision":revision}),vec!["expected_revision"]),false),
  ("export_video","Start asynchronous H.264/AAC MP4 export to a NEW absolute path inside the authorized folder. Poll get_session for completion. One video track plus independent audio tracks supported.",object(json!({"expected_revision":revision,"output_path":string}),vec!["expected_revision","output_path"]),false),
  ("cancel_export","Request cancellation of current export; original media remains unchanged.",object(json!({}),vec![]),false),
 ] {tools.push(json!({"name":name,"description":description,"inputSchema":schema,"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"openWorldHint":false}}));}
    tools
}
