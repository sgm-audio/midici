//! Golden exchange transcript replay harness. // ARD §8 / Phase 3 DoD

use std::fs;
use std::path::{Path, PathBuf};

use midici_core::{CapFlags, CiConfig, CiEngine, DeviceIdentity, Muid};
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::parse_hex_golden;

/// One step in an exchange transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranscriptStep {
    /// Feed inbound SysEx body (F0/F7 stripped) on `group`.
    Inbound { group: u8, body: Vec<u8> },
    /// Expect the next outbound body to equal `body` (group checked when present).
    ExpectOutbound { group: Option<u8>, body: Vec<u8> },
    /// Call `poll(now_ms)`.
    Poll { now_ms: u64 },
}

/// Parsed exchange transcript with engine construction knobs.
#[derive(Clone, Debug)]
pub struct ExchangeTranscript {
    pub name: String,
    pub seed: u64,
    pub config: CiConfig,
    pub steps: Vec<TranscriptStep>,
}

/// Parse a `.transcript` file.
///
/// Grammar (line-oriented):
/// - `#` comments / blank lines ignored
/// - `SEED <u64>`
/// - `MAX_SYSEX <u32>`
/// - `CAPS <hex u8>`
/// - `FB <hex u8>`
/// - `PATH <hex u8>`
/// - `GROUP <u8>` default inbound/outbound UMP group for subsequent steps
/// - `IDENTITY <m0> <m1> <m2> <family> <model> <r0> <r1> <r2> <r3>` (decimal family/model)
/// - `> [group] <hex...>` inbound
/// - `< [group] <hex...>` expected outbound
/// - `POLL <now_ms>`
pub fn parse_transcript(name: &str, text: &str) -> Result<ExchangeTranscript, String> {
    let mut seed = 1u64;
    let mut identity = DeviceIdentity {
        manufacturer: [0x43, 0, 0],
        family: 3,
        model: 4,
        software_revision: [2, 0, 0, 0],
    };
    let mut max_sysex = 512u32;
    let mut caps = CapFlags(0x08);
    let mut fb = 0x7Fu8;
    let mut path = 0u8;
    let mut default_group = 0u8;
    let mut steps = Vec::new();

    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let tag = parts
            .next()
            .ok_or_else(|| format!("line {}: empty", lineno + 1))?;
        match tag {
            "SEED" => {
                let s = parts
                    .next()
                    .ok_or_else(|| format!("line {}: SEED needs value", lineno + 1))?;
                seed = parse_u64(s).map_err(|e| format!("line {}: {e}", lineno + 1))?;
            }
            "MAX_SYSEX" => {
                let s = parts
                    .next()
                    .ok_or_else(|| format!("line {}: MAX_SYSEX needs value", lineno + 1))?;
                max_sysex = s
                    .parse()
                    .map_err(|e| format!("line {}: MAX_SYSEX: {e}", lineno + 1))?;
            }
            "CAPS" => {
                let s = parts
                    .next()
                    .ok_or_else(|| format!("line {}: CAPS needs value", lineno + 1))?;
                caps = CapFlags(parse_u8_hex(s).map_err(|e| format!("line {}: {e}", lineno + 1))?);
            }
            "FB" => {
                let s = parts
                    .next()
                    .ok_or_else(|| format!("line {}: FB needs value", lineno + 1))?;
                fb = parse_u8_hex(s).map_err(|e| format!("line {}: {e}", lineno + 1))?;
            }
            "PATH" => {
                let s = parts
                    .next()
                    .ok_or_else(|| format!("line {}: PATH needs value", lineno + 1))?;
                path = parse_u8_hex(s).map_err(|e| format!("line {}: {e}", lineno + 1))?;
            }
            "GROUP" => {
                let s = parts
                    .next()
                    .ok_or_else(|| format!("line {}: GROUP needs value", lineno + 1))?;
                default_group = s
                    .parse()
                    .map_err(|e| format!("line {}: GROUP: {e}", lineno + 1))?;
            }
            "IDENTITY" => {
                let vals: Vec<&str> = parts.collect();
                if vals.len() != 9 {
                    return Err(format!(
                        "line {}: IDENTITY needs 9 fields, got {}",
                        lineno + 1,
                        vals.len()
                    ));
                }
                identity.manufacturer = [
                    parse_u8_hex(vals[0]).map_err(|e| format!("line {}: {e}", lineno + 1))?,
                    parse_u8_hex(vals[1]).map_err(|e| format!("line {}: {e}", lineno + 1))?,
                    parse_u8_hex(vals[2]).map_err(|e| format!("line {}: {e}", lineno + 1))?,
                ];
                identity.family = vals[3]
                    .parse()
                    .map_err(|e| format!("line {}: family: {e}", lineno + 1))?;
                identity.model = vals[4]
                    .parse()
                    .map_err(|e| format!("line {}: model: {e}", lineno + 1))?;
                identity.software_revision = [
                    parse_u8_hex(vals[5]).map_err(|e| format!("line {}: {e}", lineno + 1))?,
                    parse_u8_hex(vals[6]).map_err(|e| format!("line {}: {e}", lineno + 1))?,
                    parse_u8_hex(vals[7]).map_err(|e| format!("line {}: {e}", lineno + 1))?,
                    parse_u8_hex(vals[8]).map_err(|e| format!("line {}: {e}", lineno + 1))?,
                ];
            }
            ">" | "<" => {
                let rest: Vec<&str> = parts.collect();
                if rest.is_empty() {
                    return Err(format!("line {}: missing hex", lineno + 1));
                }
                let body = parse_hex_golden(&rest.join(" "))
                    .map_err(|e| format!("line {}: {e}", lineno + 1))?;
                if tag == ">" {
                    steps.push(TranscriptStep::Inbound {
                        group: default_group,
                        body,
                    });
                } else {
                    steps.push(TranscriptStep::ExpectOutbound {
                        group: Some(default_group),
                        body,
                    });
                }
            }
            "POLL" => {
                let s = parts
                    .next()
                    .ok_or_else(|| format!("line {}: POLL needs now_ms", lineno + 1))?;
                let now_ms = s
                    .parse()
                    .map_err(|e| format!("line {}: POLL: {e}", lineno + 1))?;
                steps.push(TranscriptStep::Poll { now_ms });
            }
            other => {
                return Err(format!("line {}: unknown tag '{other}'", lineno + 1));
            }
        }
    }

    let mut config = CiConfig::responder_default(identity);
    config.max_sysex_size = max_sysex;
    config.category_supported = caps;
    config.function_block = fb;
    config.output_path_id = path;
    config.local_group = default_group;

    Ok(ExchangeTranscript {
        name: name.to_string(),
        seed,
        config,
        steps,
    })
}

fn parse_u64(s: &str) -> Result<u64, String> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).map_err(|e| e.to_string())
    } else {
        s.parse().map_err(|e| format!("{e}"))
    }
}

fn parse_u8_hex(s: &str) -> Result<u8, String> {
    u8::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16)
        .map_err(|e| format!("bad hex byte '{s}': {e}"))
}

/// Replay a transcript: feed inbounds, assert byte-exact outbounds.
pub fn replay(t: &ExchangeTranscript) -> Result<Muid, String> {
    let mut eng = CiEngine::new(t.config.clone(), StdRng::seed_from_u64(t.seed));
    for (i, step) in t.steps.iter().enumerate() {
        match step {
            TranscriptStep::Inbound { group, body } => {
                eng.feed_sysex(*group, body)
                    .map_err(|e| format!("step {i}: feed {:?}", e))?;
            }
            TranscriptStep::ExpectOutbound { group, body } => {
                let out = eng
                    .next_outbound()
                    .ok_or_else(|| format!("step {i}: expected outbound, queue empty"))?;
                if let Some(g) = group {
                    if out.group != *g {
                        return Err(format!(
                            "step {i}: group mismatch: got {}, want {g}",
                            out.group
                        ));
                    }
                }
                if out.body.as_slice() != body.as_slice() {
                    return Err(format!(
                        "step {i}: outbound mismatch\n  got:  {}\n  want: {}",
                        hex_bytes(&out.body),
                        hex_bytes(body)
                    ));
                }
            }
            TranscriptStep::Poll { now_ms } => eng.poll(*now_ms),
        }
    }
    if let Some(extra) = eng.next_outbound() {
        return Err(format!(
            "unexpected extra outbound: {}",
            hex_bytes(&extra.body)
        ));
    }
    Ok(eng.muid())
}

fn hex_bytes(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Load all `goldens/exchanges/*.transcript` files.
pub fn load_exchange_transcripts() -> Result<Vec<(PathBuf, ExchangeTranscript)>, String> {
    let dir = Path::new(crate::GOLDENS_DIR).join("exchanges");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("read {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("transcript"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let text = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
        let name = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("exchange")
            .to_string();
        let t = parse_transcript(&name, &text)?;
        out.push((p, t));
    }
    Ok(out)
}
