//! Timed caption text supplied by agents or SRT files; rendering lives in the engine.
use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Caption {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// Sorted, non-overlapping, 1..=500 captions of 1–200 characters on at most 4 lines.
pub fn validate(captions: &mut [Caption]) -> Result<()> {
    if captions.is_empty() || captions.len() > 500 {
        return Err("Provide 1..=500 captions".into());
    }
    captions.sort_by_key(|c| c.start_ms);
    let mut end = 0;
    for c in captions.iter() {
        let text = c.text.trim();
        if text.is_empty() || text.chars().count() > 200 || text.lines().count() > 4 {
            return Err(format!(
                "Caption at {} ms: text must be 1–200 characters on ≤4 lines",
                c.start_ms
            ));
        }
        if c.end_ms <= c.start_ms || c.end_ms - c.start_ms > 60_000 {
            return Err(format!(
                "Caption at {} ms: end must be after start, ≤60 s long",
                c.start_ms
            ));
        }
        if c.start_ms < end {
            return Err(format!(
                "Caption at {} ms overlaps the previous one",
                c.start_ms
            ));
        }
        end = c.end_ms;
    }
    Ok(())
}

/// Parse SubRip (`00:00:01,000 --> 00:00:02,500`), tolerating `.` decimals and BOMs.
pub fn parse_srt(text: &str) -> Result<Vec<Caption>> {
    let time = |s: &str| -> Option<u64> {
        let s = s.trim().replace(',', ".");
        let (hms, ms) = s.split_once('.').unwrap_or((&s, "0"));
        let parts: Vec<u64> = hms
            .split(':')
            .map(|p| p.trim().parse().ok())
            .collect::<Option<_>>()?;
        let [h, m, sec] = parts[..] else { return None };
        let ms: u64 = format!("{:0<3}", &ms[..ms.len().min(3)]).parse().ok()?;
        Some(((h * 60 + m) * 60 + sec) * 1000 + ms)
    };
    let mut captions = vec![];
    for block in text
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .split("\n\n")
    {
        let lines: Vec<&str> = block
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let Some(at) = lines.iter().position(|l| l.contains("-->")) else {
            continue;
        };
        let (a, b) = lines[at].split_once("-->").ok_or("Bad SRT timing")?;
        let (start_ms, end_ms) = match (time(a), time(b.split_whitespace().next().unwrap_or(""))) {
            (Some(a), Some(b)) => (a, b),
            _ => return Err(format!("Bad SRT timing: {}", lines[at])),
        };
        let text = lines[at + 1..].join("\n");
        if !text.is_empty() {
            captions.push(Caption {
                start_ms,
                end_ms,
                text,
            });
        }
    }
    if captions.is_empty() {
        return Err("No captions found in SRT".into());
    }
    Ok(captions)
}
