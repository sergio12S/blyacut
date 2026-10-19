//! Framework-independent, transactional editing model. All times are integer milliseconds.
pub mod effects;
use effects::EffectRecipe;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;
pub type Result<T> = std::result::Result<T, String>;
pub fn id() -> String {
    Uuid::new_v4().to_string()
}
const MAX_TIME: u64 = 604_800_000; // One week, also safely representable in JS.
/// Current project schema. v4 adds still images, overlay tracks and clip transforms.
pub const VERSION: u32 = 4;
/// Usable length of a still image asset; clips trim it to the duration they need.
pub const STILL_MS: u64 = 3_600_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    /// Fill the frame by cropping (e.g. landscape footage in a vertical project) instead of letterboxing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fill: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    /// A single image (PNG/JPEG/WebP) held for any clip length.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub still: bool,
}
fn unity() -> u16 {
    1000
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Clip {
    pub id: String,
    pub asset_id: String,
    pub position_ms: u64,
    pub source_start_ms: u64,
    pub source_end_ms: u64,
    #[serde(default = "unity")]
    pub gain_milli: u16,
    #[serde(default)]
    pub fade_in_ms: u64,
    #[serde(default)]
    pub fade_out_ms: u64,
    #[serde(default)]
    pub effects: Vec<EffectRecipe>,
    /// Placement on overlay video tracks; `None` fills the frame like the main track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Transform>,
    /// Transition from the previous main-track clip into this one, centred on the cut.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_in: Option<Transition>,
    /// Playback speed in thousandths (250 = 0.25×, 4000 = 4×); pitch is preserved.
    #[serde(default = "unity", skip_serializing_if = "is_unity")]
    pub speed_milli: u16,
    /// Play the source range backwards.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reverse: bool,
}
fn is_unity(v: &u16) -> bool {
    *v == 1000
}
/// Visual transitions (FFmpeg `xfade` names); linked audio always crossfades.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Fade,
    Fadeblack,
    Fadewhite,
    Dissolve,
    Wipeleft,
    Wiperight,
    Wipeup,
    Wipedown,
    Slideleft,
    Slideright,
    Slideup,
    Slidedown,
    Smoothleft,
    Smoothright,
    Circleopen,
    Circleclose,
    Radial,
    Pixelize,
    Zoomin,
    Hblur,
}
impl TransitionKind {
    pub const ALL: [Self; 20] = [
        Self::Fade,
        Self::Fadeblack,
        Self::Fadewhite,
        Self::Dissolve,
        Self::Wipeleft,
        Self::Wiperight,
        Self::Wipeup,
        Self::Wipedown,
        Self::Slideleft,
        Self::Slideright,
        Self::Slideup,
        Self::Slidedown,
        Self::Smoothleft,
        Self::Smoothright,
        Self::Circleopen,
        Self::Circleclose,
        Self::Radial,
        Self::Pixelize,
        Self::Zoomin,
        Self::Hblur,
    ];
    /// The `xfade` transition name.
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub kind: TransitionKind,
    pub duration_ms: u32,
}
/// Overlay placement in thousandths: centre position as a fraction of the frame,
/// scale relative to fitting the frame, and opacity.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub x_milli: u16,
    pub y_milli: u16,
    pub scale_milli: u16,
    pub opacity_milli: u16,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            x_milli: 500,
            y_milli: 500,
            scale_milli: 1000,
            opacity_milli: 1000,
        }
    }
}
impl Transform {
    pub fn validate(&self) -> Result<()> {
        if self.x_milli > 1000
            || self.y_milli > 1000
            || !(10..=4000).contains(&self.scale_milli)
            || self.opacity_milli > 1000
        {
            return Err("Transform: x/y/opacity 0..=1000, scale 10..=4000".into());
        }
        Ok(())
    }
}
impl Clip {
    /// Length of the source range used.
    pub fn span(&self) -> u64 {
        self.source_end_ms.saturating_sub(self.source_start_ms)
    }
    /// Length on the timeline, after speed.
    pub fn duration(&self) -> u64 {
        if self.speed_milli == 1000 {
            return self.span();
        }
        (self.span() * 1000 + self.speed_milli as u64 / 2) / self.speed_milli.max(1) as u64
    }
    /// Source time shown `local_ms` into the clip on the timeline.
    pub fn source_at(&self, local_ms: u64) -> u64 {
        let offset = (local_ms * self.speed_milli as u64 / 1000).min(self.span());
        if self.reverse {
            self.source_end_ms - offset
        } else {
            self.source_start_ms + offset
        }
    }
    /// Normal-speed, forward clips are the only ones transitions can join (for now).
    pub fn is_retimed(&self) -> bool {
        self.speed_milli != 1000 || self.reverse
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Video,
    Audio,
    Text,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub kind: TrackKind,
    pub clips: Vec<Clip>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub settings: Settings,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            version: VERSION,
            id: id(),
            name: "Untitled project".into(),
            settings: Settings {
                width: 1920,
                height: 1080,
                fps_num: 30,
                fps_den: 1,
                fill: false,
            },
            assets: vec![],
            tracks: vec![
                Track {
                    id: id(),
                    name: "Video 1".into(),
                    kind: TrackKind::Video,
                    clips: vec![],
                },
                Track {
                    id: id(),
                    name: "Audio 1".into(),
                    kind: TrackKind::Audio,
                    clips: vec![],
                },
            ],
        }
    }
}
impl Project {
    /// The first video track is the magnetic main track; later video tracks are overlays.
    pub fn main_video_track(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Video)
    }
    pub fn is_overlay_track(&self, track: &Track) -> bool {
        track.kind == TrackKind::Video && self.main_video_track().is_some_and(|m| m.id != track.id)
    }
    pub fn duration(&self) -> u64 {
        self.tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(|c| c.position_ms + c.duration())
            .max()
            .unwrap_or(0)
    }
    /// Plain-language notes on how export will treat this timeline, for agents and the UI.
    pub fn warnings(&self) -> Vec<String> {
        let secs = |ms: u64| format!("{:.2} s", ms as f64 / 1000.0);
        let end = self.duration();
        if end == 0 {
            return vec!["Timeline is empty".into()];
        }
        let mut out = vec![];
        let overlay_sound = self
            .tracks
            .iter()
            .filter(|t| self.is_overlay_track(t))
            .flat_map(|t| &t.clips)
            .any(|c| {
                self.assets
                    .iter()
                    .any(|a| a.id == c.asset_id && a.has_audio)
            });
        if overlay_sound {
            out.push(
                "Overlay clips export without sound; put their audio on an audio track".into(),
            );
        }
        let Some(track) = self.main_video_track().filter(|t| !t.clips.is_empty()) else {
            out.push(format!(
                "Main video track is empty: export is {} of black frames under any overlays",
                secs(end)
            ));
            return out;
        };
        let mut cursor = 0;
        for c in &track.clips {
            if c.position_ms > cursor {
                out.push(format!(
                    "Video gap {}–{} exports as black frames",
                    secs(cursor),
                    secs(c.position_ms)
                ));
            }
            cursor = c.position_ms + c.duration();
        }
        if end > cursor {
            out.push(format!(
                "Audio runs {} past the last video clip ({}–{}): black frames; trim/fade the audio or add video",
                secs(end - cursor),
                secs(cursor),
                secs(end)
            ));
        }
        out
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != VERSION {
            return Err(format!("Unsupported project version {}", self.version));
        }
        if self.assets.len() > 4096
            || self.tracks.len() > 32
            || self.tracks.iter().map(|t| t.clips.len()).sum::<usize>() > 4096
        {
            return Err("Project exceeds asset, track or clip limits".into());
        }
        let s = &self.settings;
        if s.width < 2
            || s.height < 2
            || s.width > 7680
            || s.height > 4320
            || !s.width.is_multiple_of(2)
            || !s.height.is_multiple_of(2)
            || s.fps_num == 0
            || s.fps_den == 0
            || s.fps_num > 240 * s.fps_den.min(1000)
            || s.fps_den > 1000
        {
            return Err("Invalid output settings".into());
        }
        let mut ids = HashSet::new();
        if self.id.is_empty() || !ids.insert(self.id.as_str()) {
            return Err("Invalid project ID".into());
        }
        for a in &self.assets {
            if a.id.is_empty()
                || !ids.insert(&a.id)
                || a.duration_ms == 0
                || a.duration_ms > MAX_TIME
                || (a.width == 0) != (a.height == 0)
                || (a.width == 0 && !a.has_audio)
                || (a.still && (a.width == 0 || a.has_audio))
            {
                return Err("Invalid or duplicate asset".into());
            }
        }
        if self.tracks.is_empty() {
            return Err("Project needs a track".into());
        }
        let main = self.main_video_track().map(|t| t.id.as_str());
        for t in &self.tracks {
            if t.id.is_empty() || !ids.insert(&t.id) {
                return Err("Duplicate track ID".into());
            }
            let mut end = 0;
            // Previous clip, its asset, and how much of its start a transition consumes.
            let mut previous: Option<(&Clip, &Asset, u64)> = None;
            for c in &t.clips {
                let a = self
                    .assets
                    .iter()
                    .find(|a| a.id == c.asset_id)
                    .ok_or("Clip references unknown asset")?;
                if c.id.is_empty()
                    || !ids.insert(&c.id)
                    || c.source_start_ms >= c.source_end_ms
                    || c.source_end_ms > a.duration_ms
                    || c.position_ms > MAX_TIME
                    || c.duration() > MAX_TIME - c.position_ms
                {
                    return Err("Invalid clip range or ID".into());
                }
                if c.effects.len() > 8
                    || c.effects.iter().map(|r| r.steps.len()).sum::<usize>() > 32
                {
                    return Err("Clip effect chain exceeds limits".into());
                }
                for recipe in &c.effects {
                    recipe.validate()?;
                    if t.kind == TrackKind::Audio && recipe.steps.iter().any(|s| s.is_video()) {
                        return Err("Video effect cannot be applied to an audio-only track".into());
                    }
                }
                if let Some(transform) = &c.transform {
                    transform.validate()?;
                    if !self.is_overlay_track(t) {
                        return Err(
                            "Transforms apply to clips on overlay video tracks (V2+)".into()
                        );
                    }
                }
                if !(250..=4000).contains(&c.speed_milli) {
                    return Err("Clip speed must be 250..=4000 (0.25×–4×)".into());
                }
                if a.still && c.is_retimed() {
                    return Err("Still images have no speed or direction".into());
                }
                if c.gain_milli > 1000
                    || c.fade_in_ms > c.duration()
                    || c.fade_out_ms > c.duration() - c.fade_in_ms
                {
                    return Err("Invalid clip gain or fades".into());
                }
                if (t.kind == TrackKind::Video && a.width == 0)
                    || (t.kind == TrackKind::Audio && !a.has_audio)
                    || t.kind == TrackKind::Text
                {
                    return Err("Media is incompatible with track type".into());
                }
                if c.position_ms < end {
                    return Err("Clips overlap or are out of order on a track".into());
                }
                let mut consumed_in = 0;
                if let Some(tr) = &c.transition_in {
                    if !(100..=5000).contains(&tr.duration_ms) {
                        return Err("Transition duration must be 100..=5000 ms".into());
                    }
                    let Some((prev, prev_asset, prev_in)) = previous.filter(|(p, ..)| {
                        main == Some(t.id.as_str()) && p.position_ms + p.duration() == c.position_ms
                    }) else {
                        return Err("A transition needs the clip to directly follow another main-track clip".into());
                    };
                    if prev.is_retimed() || c.is_retimed() {
                        return Err("Transitions next to speed-changed or reversed clips are not supported yet".into());
                    }
                    let (before, after) = transition_halves(tr);
                    if (!prev_asset.still && prev.source_end_ms + after > prev_asset.duration_ms)
                        || (!a.still && c.source_start_ms < before)
                    {
                        return Err(format!(
                            "A {} ms transition needs {} ms of media after the outgoing clip's end and {} ms before the incoming clip's start; trim the clips or shorten the transition",
                            tr.duration_ms, after, before
                        ));
                    }
                    if prev.duration() <= prev_in + before || c.duration() <= after {
                        return Err("Transition is longer than the clips around it".into());
                    }
                    consumed_in = after;
                }
                previous = Some((c, a, consumed_in));
                end = c.position_ms + c.duration();
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    SetEffects {
        clip_id: String,
        effects: Vec<EffectRecipe>,
    },
    /// Change the output frame, rate and letterbox/fill mode.
    SetSettings {
        settings: Settings,
    },
    /// Transition into a main-track clip from the clip before it; `null` removes it.
    SetTransition {
        clip_id: String,
        transition: Option<Transition>,
        /// Trim the outgoing end / incoming start just enough to create the needed handles.
        #[serde(default)]
        make_room: bool,
    },
    /// Retime a clip: speed in thousandths (250..=4000) and direction.
    SetSpeed {
        clip_id: String,
        speed_milli: u16,
        #[serde(default)]
        reverse: bool,
    },
    /// Place an overlay clip; `null` resets it to fill the frame.
    SetTransform {
        clip_id: String,
        transform: Option<Transform>,
    },
    AddTrack {
        track_id: String,
        name: String,
        kind: TrackKind,
    },
    MoveClip {
        clip_id: String,
        track_id: String,
        position_ms: u64,
    },
    SetAudio {
        clip_id: String,
        gain_milli: u16,
        fade_in_ms: u64,
        fade_out_ms: u64,
    },
    Append {
        asset_id: String,
        clip_id: String,
        track_id: String,
    },
    SplitClip {
        clip_id: String,
        position_ms: u64,
        new_clip_id: String,
    },
    TrimClip {
        clip_id: String,
        start_ms: u64,
        end_ms: u64,
    },
    DeleteClip {
        clip_id: String,
    },
    ReorderClip {
        clip_id: String,
        index: usize,
    },
}
/// Drop transitions whose clip no longer directly follows another main-track clip.
fn settle_transitions(p: &mut Project) {
    let main = p.main_video_track().map(|t| t.id.clone());
    for t in &mut p.tracks {
        let is_main = main.as_deref() == Some(t.id.as_str());
        let mut previous_end = None;
        for c in &mut t.clips {
            if c.transition_in.is_some() && (!is_main || previous_end != Some(c.position_ms)) {
                c.transition_in = None;
            }
            previous_end = Some(c.position_ms + c.duration());
        }
    }
}
/// Split of a transition around the cut: (before the cut, after the cut).
pub fn transition_halves(t: &Transition) -> (u64, u64) {
    let d = t.duration_ms as u64;
    (d / 2, d - d / 2)
}
fn ripple(t: &mut Track) {
    let mut pos = 0;
    for c in &mut t.clips {
        c.position_ms = pos;
        pos += c.duration();
    }
}
fn apply(p: &mut Project, edit: Edit) -> Result<()> {
    let main_video = p.main_video_track().map(|t| t.id.clone());
    match &edit {
        Edit::AddTrack {
            track_id,
            name,
            kind,
        } => {
            if *kind == TrackKind::Text
                || name.is_empty()
                || name.len() > 128
                || p.tracks.len() >= 32
            {
                return Err("Invalid track or track limit reached".into());
            }
            p.tracks.push(Track {
                id: track_id.clone(),
                name: name.clone(),
                kind: kind.clone(),
                clips: vec![],
            });
            return p.validate();
        }
        Edit::SetSettings { settings } => {
            p.settings = settings.clone();
            return p.validate();
        }
        Edit::MoveClip {
            clip_id,
            track_id,
            position_ms,
        } => {
            let source = p
                .tracks
                .iter()
                .position(|t| t.clips.iter().any(|c| &c.id == clip_id))
                .ok_or("Unknown clip")?;
            let target = p
                .tracks
                .iter()
                .position(|t| &t.id == track_id)
                .ok_or("Unknown track")?;
            let index = p.tracks[source]
                .clips
                .iter()
                .position(|c| &c.id == clip_id)
                .ok_or("Unknown clip")?;
            let mut clip = p.tracks[source].clips.remove(index);
            clip.position_ms = *position_ms;
            p.tracks[target].clips.push(clip);
            p.tracks[target].clips.sort_by_key(|c| c.position_ms);
            settle_transitions(p);
            return p.validate();
        }
        _ => {}
    }
    if let Edit::Append {
        asset_id,
        clip_id,
        track_id,
    } = edit
    {
        let a = p
            .assets
            .iter()
            .find(|a| a.id == asset_id)
            .ok_or("Unknown asset")?;
        let t = p
            .tracks
            .iter_mut()
            .find(|t| t.id == track_id)
            .ok_or("Unknown track")?;
        let pos = t
            .clips
            .last()
            .map(|c| c.position_ms + c.duration())
            .unwrap_or(0);
        t.clips.push(Clip {
            id: clip_id,
            asset_id,
            position_ms: pos,
            source_start_ms: 0,
            source_end_ms: a.duration_ms,
            gain_milli: 1000,
            fade_in_ms: 0,
            fade_out_ms: 0,
            effects: vec![],
            transform: None,
            transition_in: None,
            speed_milli: 1000,
            reverse: false,
        });
    } else {
        let cid = match &edit {
            Edit::SplitClip { clip_id, .. }
            | Edit::TrimClip { clip_id, .. }
            | Edit::DeleteClip { clip_id }
            | Edit::ReorderClip { clip_id, .. }
            | Edit::SetAudio { clip_id, .. }
            | Edit::SetEffects { clip_id, .. }
            | Edit::SetTransform { clip_id, .. }
            | Edit::SetTransition { clip_id, .. }
            | Edit::SetSpeed { clip_id, .. } => clip_id,
            _ => unreachable!(),
        };
        let t = p
            .tracks
            .iter_mut()
            .find(|t| t.clips.iter().any(|c| &c.id == cid))
            .ok_or("Unknown clip")?;
        let i = t
            .clips
            .iter()
            .position(|c| &c.id == cid)
            .ok_or("Unknown clip")?;
        // Only the main video track is magnetic; overlays stay where they were placed.
        let ripples = main_video.as_deref() == Some(t.id.as_str());
        match edit {
            Edit::SplitClip {
                position_ms,
                new_clip_id,
                ..
            } => {
                let c = &mut t.clips[i];
                if position_ms <= c.position_ms || position_ms >= c.position_ms + c.duration() {
                    return Err("Split must be inside the clip (timeline milliseconds)".into());
                }
                let offset = position_ms - c.position_ms;
                if offset < c.fade_in_ms || c.duration() - offset < c.fade_out_ms {
                    return Err("Cannot split inside a fade; adjust or clear the fade first".into());
                }
                // The source point shown at the cut; reversed clips consume the source backwards.
                let cut = c.source_at(offset);
                let (right_start, right_end) = if c.reverse {
                    (c.source_start_ms, cut)
                } else {
                    (cut, c.source_end_ms)
                };
                let mut right = Clip {
                    id: new_clip_id,
                    asset_id: c.asset_id.clone(),
                    position_ms,
                    source_start_ms: right_start,
                    source_end_ms: right_end,
                    gain_milli: c.gain_milli,
                    fade_in_ms: 0,
                    fade_out_ms: c.fade_out_ms,
                    effects: c.effects.clone(),
                    transform: c.transform,
                    transition_in: None,
                    speed_milli: c.speed_milli,
                    reverse: c.reverse,
                };
                if right.source_start_ms >= right.source_end_ms
                    || cut == c.source_start_ms
                    || cut == c.source_end_ms
                {
                    return Err("Split must be inside the clip (timeline milliseconds)".into());
                }
                if c.reverse {
                    c.source_start_ms = cut;
                } else {
                    c.source_end_ms = cut;
                }
                c.fade_in_ms = c.fade_in_ms.min(c.duration());
                c.fade_out_ms = 0;
                // Speed rounding: the right part starts exactly where the left one now ends.
                right.position_ms = c.position_ms + c.duration();
                right.fade_out_ms = right.fade_out_ms.min(right.duration());
                t.clips.insert(i + 1, right);
            }
            Edit::TrimClip {
                start_ms, end_ms, ..
            } => {
                let c = &mut t.clips[i];
                if start_ms >= end_ms {
                    return Err("Trim needs a positive source range".into());
                }
                c.source_start_ms = start_ms;
                c.source_end_ms = end_ms;
                if ripples {
                    ripple(t);
                }
            }
            Edit::DeleteClip { .. } => {
                t.clips.remove(i);
                if ripples {
                    ripple(t);
                }
            }
            Edit::SetEffects { effects, .. } => {
                t.clips[i].effects = effects;
            }
            Edit::SetTransform { transform, .. } => {
                t.clips[i].transform = transform;
            }
            Edit::SetSpeed {
                speed_milli,
                reverse,
                ..
            } => {
                let c = &mut t.clips[i];
                c.speed_milli = speed_milli;
                c.reverse = reverse;
                c.fade_in_ms = c.fade_in_ms.min(c.duration());
                c.fade_out_ms = c.fade_out_ms.min(c.duration() - c.fade_in_ms);
                if ripples {
                    ripple(t);
                } else if let Some(next) = t.clips.get(i + 1) {
                    if t.clips[i].position_ms + t.clips[i].duration() > next.position_ms {
                        return Err("Slower clip would overlap the next clip on this track".into());
                    }
                }
            }
            Edit::SetTransition {
                transition,
                make_room,
                ..
            } => {
                if !ripples {
                    return Err("Transitions join clips on the main video track".into());
                }
                if transition.is_some() && i == 0 {
                    return Err("The first clip has no previous clip to transition from".into());
                }
                if let (Some(tr), true) = (&transition, make_room) {
                    let (before, after) = transition_halves(tr);
                    let still = |id: &str| {
                        p.assets
                            .iter()
                            .find(|a| a.id == id)
                            .map(|a| (a.still, a.duration_ms))
                            .unwrap_or((false, 0))
                    };
                    let (prev_still, prev_len) = still(&t.clips[i - 1].asset_id);
                    let (cur_still, _) = still(&t.clips[i].asset_id);
                    let prev = &mut t.clips[i - 1];
                    if !prev_still {
                        let deficit = (prev.source_end_ms + after).saturating_sub(prev_len);
                        prev.source_end_ms = prev.source_end_ms.saturating_sub(deficit);
                    }
                    let cur = &mut t.clips[i];
                    if !cur_still && cur.source_start_ms < before {
                        cur.source_start_ms = before;
                    }
                    for c in [i - 1, i] {
                        let c = &mut t.clips[c];
                        if c.source_start_ms >= c.source_end_ms {
                            return Err("Clip is too short to make room for this transition".into());
                        }
                        c.fade_in_ms = c.fade_in_ms.min(c.duration());
                        c.fade_out_ms = c.fade_out_ms.min(c.duration() - c.fade_in_ms);
                    }
                    ripple(t);
                }
                t.clips[i].transition_in = transition;
            }
            Edit::SetAudio {
                gain_milli,
                fade_in_ms,
                fade_out_ms,
                ..
            } => {
                let c = &mut t.clips[i];
                c.gain_milli = gain_milli;
                c.fade_in_ms = fade_in_ms;
                c.fade_out_ms = fade_out_ms;
            }
            Edit::ReorderClip { index, .. } => {
                if !ripples {
                    return Err("Use move_clip to position audio and overlay clips".into());
                }
                if index >= t.clips.len() {
                    return Err("Invalid clip index".into());
                }
                let c = t.clips.remove(i);
                t.clips.insert(index, c);
                if ripples {
                    ripple(t);
                }
            }
            _ => unreachable!(),
        }
    }
    settle_transitions(p);
    p.validate()
}
#[derive(Default)]
pub struct Editor {
    pub project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    pub revision: u64,
}
impl Editor {
    pub fn new(project: Project) -> Result<Self> {
        project.validate()?;
        Ok(Self {
            project,
            ..Self::default()
        })
    }
    fn commit(&mut self, p: Project) {
        self.undo.push(std::mem::replace(&mut self.project, p));
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
    }
    pub fn preview(&self, edits: &[Edit]) -> Result<Project> {
        if edits.is_empty() || edits.len() > 100 {
            return Err("Batch must contain 1–100 edits".into());
        }
        let mut next = self.project.clone();
        for edit in edits {
            apply(&mut next, edit.clone())?;
        }
        Ok(next)
    }
    pub fn execute(&mut self, edit: Edit) -> Result<()> {
        self.batch(&[edit], self.revision)
    }
    pub fn batch(&mut self, edits: &[Edit], expected_revision: u64) -> Result<()> {
        if expected_revision != self.revision {
            return Err(format!(
                "Stale revision: expected {}, current {}",
                expected_revision, self.revision
            ));
        }
        let next = self.preview(edits)?;
        self.commit(next);
        Ok(())
    }
    pub fn replace(&mut self, project: Project) -> Result<()> {
        project.validate()?;
        self.project = project;
        self.undo.clear();
        self.redo.clear();
        self.revision += 1;
        Ok(())
    }
    pub fn import(&mut self, asset: Asset) -> Result<()> {
        let mut next = self.project.clone();
        next.assets.push(asset);
        next.validate()?;
        self.commit(next);
        Ok(())
    }
    pub fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, p));
            self.revision += 1;
        }
    }
    pub fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, p));
            self.revision += 1;
        }
    }
}

pub fn save(project: &Project, path: &std::path::Path) -> Result<()> {
    project.validate()?;
    if path.extension().and_then(|s| s.to_str()) != Some("blyacut") {
        return Err("Project filename must end in .blyacut".into());
    }
    let parent = path.parent().ok_or("Project needs a parent directory")?;
    let mut portable = project.clone();
    for a in &mut portable.assets {
        if let Ok(relative) = a.path.strip_prefix(parent) {
            a.path = relative.into();
        }
    }
    let temp = parent.join(format!(".blyacut-{}.tmp", id()));
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(&portable).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(temp);
        return Err(e.to_string());
    }
    Ok(())
}
pub fn load(path: &std::path::Path) -> Result<Project> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 16 * 1024 * 1024 {
        return Err("Project exceeds 16 MiB".into());
    }
    let mut p: Project = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if (1..VERSION).contains(&p.version) {
        p.version = VERSION;
    }
    p.validate()?;
    for a in &mut p.assets {
        if a.path.is_relative() {
            a.path = path.parent().ok_or("Missing parent")?.join(&a.path);
        }
    }
    Ok(p)
}
