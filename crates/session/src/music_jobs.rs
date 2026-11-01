//! A bounded, sequential queue for standalone music release exports.
use blyacut_core::{id, Result};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone)]
pub struct Request {
    pub input: PathBuf,
    pub output_dir: PathBuf,
    pub prefix: String,
    pub title: String,
    pub seconds: u32,
    pub starts_ms: Option<Vec<u64>>,
    pub srt: Option<String>,
    pub look: Option<String>,
    pub intensity: u32,
    pub caption_mode: String,
}

pub fn execute(
    request: &Request,
    cancel: &AtomicBool,
    progress: impl Fn(u8),
) -> Result<serde_json::Value> {
    let mut input = request.input.clone();
    let mut offset = 0;
    if let Some(look) = &request.look {
        let styled = request
            .output_dir
            .join(format!("{}-styled.mp4", request.prefix));
        let opts = blyacut_engine::visualizer::PhonkOptions {
            look: look.clone(),
            ..Default::default()
        };
        blyacut_engine::visualizer::render_phonk_controlled(
            &input,
            &styled,
            request.intensity,
            &opts,
            cancel,
        )?;
        input = styled;
        offset = 1;
        progress(offset);
    }
    let result = blyacut_engine::release::render_controlled(
        &input,
        &request.output_dir,
        &request.prefix,
        &request.title,
        request.seconds,
        request.starts_ms.clone(),
        request.srt.as_deref(),
        &request.caption_mode,
        cancel,
        |n| progress(n + offset),
    )?;
    let mut value = serde_json::to_value(result).map_err(|e| e.to_string())?;
    if offset == 1 {
        value["visualizer_path"] = serde_json::json!(input);
    }
    Ok(value)
}
#[derive(Clone, Serialize)]
pub struct Status {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub completed_steps: u8,
    pub total_steps: u8,
    pub input: PathBuf,
    pub prefix: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}
struct Entry {
    request: Work,
    state: Status,
    cancel: Arc<AtomicBool>,
}
#[derive(Clone)]
enum Work {
    Release(Request),
    Loop(blyacut_engine::loopmix::Request),
}
impl Work {
    fn outputs(&self) -> Vec<PathBuf> {
        match self {
            Work::Loop(r) => vec![r.output.clone()],
            Work::Release(r) => ["full", "short-1", "short-2", "short-3", "styled"]
                .iter()
                .map(|s| r.output_dir.join(format!("{}-{s}.mp4", r.prefix)))
                .collect(),
        }
    }
    fn label(&self) -> (PathBuf, String, u8, &'static str) {
        match self {
            Work::Loop(r) => (
                r.segments[0].path.clone(),
                r.output
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                2,
                "loop_mix",
            ),
            Work::Release(r) => (
                r.input.clone(),
                r.prefix.clone(),
                if r.look.is_some() { 5 } else { 4 },
                "release",
            ),
        }
    }
}
#[derive(Default)]
struct State {
    entries: HashMap<String, Entry>,
    pending: VecDeque<String>,
    worker_running: bool,
}
#[derive(Default)]
pub struct MusicQueue {
    inner: Mutex<State>,
}

impl MusicQueue {
    pub fn submit(self: &Arc<Self>, requests: Vec<Request>) -> Result<Vec<Status>> {
        self.submit_work(requests.into_iter().map(Work::Release).collect())
    }
    pub fn submit_loop(
        self: &Arc<Self>,
        request: blyacut_engine::loopmix::Request,
    ) -> Result<Status> {
        blyacut_engine::loopmix::validate(&request)?;
        self.submit_work(vec![Work::Loop(request)])
            .map(|mut jobs| jobs.remove(0))
    }
    fn submit_work(self: &Arc<Self>, requests: Vec<Work>) -> Result<Vec<Status>> {
        if requests.is_empty() || requests.len() > 10 {
            return Err("Submit 1..=10 release jobs".into());
        }
        let mut state = self.inner.lock().map_err(|e| e.to_string())?;
        let active = state
            .entries
            .values()
            .filter(|e| e.state.status == "queued" || e.state.status == "running")
            .count();
        if active + requests.len() > 16 {
            return Err("Music queue is full (maximum 16 active jobs)".into());
        }
        if state.entries.len() + requests.len() > 100 {
            return Err("Music job history is full; restart the session".into());
        }
        for (i, req) in requests.iter().enumerate() {
            let paths = req.outputs();
            let duplicate = |r: &Work| r.outputs().iter().any(|p| paths.contains(p));
            if requests[..i].iter().any(duplicate)
                || state.entries.values().any(|e| {
                    (e.state.status == "running" || e.state.status == "queued")
                        && duplicate(&e.request)
                })
            {
                return Err("Duplicate output directory/prefix in active jobs".into());
            }
        }
        let mut added = vec![];
        for req in requests {
            let (input, prefix, total_steps, kind) = req.label();
            let job = Status {
                id: id(),
                kind: kind.into(),
                status: "queued".into(),
                completed_steps: 0,
                total_steps,
                input,
                prefix,
                result: None,
                error: None,
            };
            state.pending.push_back(job.id.clone());
            state.entries.insert(
                job.id.clone(),
                Entry {
                    request: req,
                    state: job.clone(),
                    cancel: Arc::new(AtomicBool::new(false)),
                },
            );
            added.push(job);
        }
        if !state.worker_running {
            state.worker_running = true;
            let queue = self.clone();
            std::thread::spawn(move || queue.work());
        }
        Ok(added)
    }
    fn work(self: Arc<Self>) {
        loop {
            let (id, request, cancel) = {
                let Ok(mut state) = self.inner.lock() else {
                    return;
                };
                let Some(id) = state.pending.pop_front() else {
                    state.worker_running = false;
                    return;
                };
                let Some(entry) = state.entries.get_mut(&id) else {
                    continue;
                };
                if entry.cancel.load(Ordering::Relaxed) {
                    entry.state.status = "cancelled".into();
                    continue;
                }
                entry.state.status = "running".into();
                (id, entry.request.clone(), entry.cancel.clone())
            };
            let progress_queue = self.clone();
            let progress_id = id.clone();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let report = move |step| {
                    if let Ok(mut state) = progress_queue.inner.lock() {
                        if let Some(e) = state.entries.get_mut(&progress_id) {
                            e.state.completed_steps = step;
                        }
                    }
                };
                match &request {
                    Work::Release(request) => execute(request, &cancel, report),
                    Work::Loop(request) => {
                        blyacut_engine::loopmix::render(request, &cancel, report)
                            .and_then(|r| serde_json::to_value(r).map_err(|e| e.to_string()))
                    }
                }
            }))
            .unwrap_or_else(|_| {
                Err("Music job failed unexpectedly; partial output may remain".into())
            });
            if let Ok(mut state) = self.inner.lock() {
                if let Some(e) = state.entries.get_mut(&id) {
                    match outcome {
                        Ok(result) => {
                            e.state.status = "complete".into();
                            e.state.result = Some(result);
                        }
                        Err(err) => {
                            e.state.status = if cancel.load(Ordering::Relaxed) {
                                "cancelled"
                            } else {
                                "failed"
                            }
                            .into();
                            e.state.error = Some(err);
                        }
                    }
                }
            }
        }
    }
    pub fn list(&self) -> Result<Vec<Status>> {
        let state = self.inner.lock().map_err(|e| e.to_string())?;
        let mut jobs: Vec<_> = state.entries.values().map(|e| e.state.clone()).collect();
        jobs.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(jobs)
    }
    pub fn cancel(&self, job_id: &str) -> Result<Status> {
        let mut state = self.inner.lock().map_err(|e| e.to_string())?;
        let entry = state.entries.get_mut(job_id).ok_or("Unknown music job")?;
        if entry.state.status != "queued" && entry.state.status != "running" {
            return Err("Music job already finished".into());
        }
        entry.cancel.store(true, Ordering::Relaxed);
        if entry.state.status == "queued" {
            entry.state.status = "cancelled".into();
        }
        Ok(entry.state.clone())
    }
    pub fn wait(&self) {
        while self.inner.lock().map(|s| s.worker_running).unwrap_or(false) {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}
