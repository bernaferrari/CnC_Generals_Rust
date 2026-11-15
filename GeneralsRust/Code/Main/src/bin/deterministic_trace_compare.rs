use anyhow::{Context, Result, bail};
#[cfg(test)]
use generals_main::deterministic_trace::TraceDifference;
use generals_main::deterministic_trace::{
    FrameTrace, TraceFieldDifference, first_trace_field_difference,
};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Args {
    left: PathBuf,
    right: PathBuf,
}

#[derive(Debug, Deserialize)]
struct TraceInput {
    schema: String,
    scenario: String,
    frames: Vec<FrameTrace>,
}

fn main() -> Result<()> {
    let args = Args::parse(env::args().skip(1))?;
    let left = read_trace(&args.left)?;
    let right = read_trace(&args.right)?;

    if left.scenario != right.scenario {
        bail!(
            "scenario mismatch: left={:?}, right={:?}",
            left.scenario,
            right.scenario
        );
    }
    if let Some(difference) = first_trace_field_difference(&left.frames, &right.frames) {
        bail!("{}", format_field_difference(&difference));
    }
    println!("traces match: {} frames", left.frames.len());
    Ok(())
}

fn format_field_difference(difference: &TraceFieldDifference) -> String {
    format!(
        "trace mismatch at index {} (left frame {}, right frame {}): {} at {}: left={}, right={}",
        difference.index,
        difference.left_frame,
        difference.right_frame,
        difference.category,
        difference.path,
        difference.left,
        difference.right,
    )
}

impl Args {
    fn parse<I>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = String>,
    {
        let mut paths = Vec::new();

        for arg in args {
            match arg.as_str() {
                "--help" | "-h" => {
                    println!("Usage: deterministic_trace_compare <left.json> <right.json>");
                    std::process::exit(0);
                }
                other if other.starts_with('-') => bail!("unknown argument '{other}'"),
                _ => paths.push(PathBuf::from(arg)),
            }
        }

        if paths.len() != 2 {
            bail!("expected exactly two trace JSON paths");
        }

        Ok(Self {
            left: paths.remove(0),
            right: paths.remove(0),
        })
    }
}

fn read_trace(path: &Path) -> Result<TraceInput> {
    let json = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_trace(&json).with_context(|| format!("parsing {}", path.display()))
}

fn parse_trace(json: &str) -> Result<TraceInput> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    if let Some(frames) = value.get("frames").and_then(serde_json::Value::as_array) {
        for (index, frame) in frames.iter().enumerate() {
            for field in ["players", "events", "xfer_bytes"] {
                if frame.get(field).is_none() {
                    bail!("frame {index} is missing required v2 field {field:?}");
                }
            }
        }
    }
    let input: TraceInput = serde_json::from_value(value)?;
    if input.schema != "generals.frame_trace.v2" {
        bail!("unsupported trace schema {:?}", input.schema);
    }
    if input.scenario.is_empty() {
        bail!("trace scenario must not be empty");
    }
    if input.frames.is_empty() {
        bail!("trace contains no frames");
    }
    for pair in input.frames.windows(2) {
        if pair[1].frame <= pair[0].frame {
            bail!("trace frames must be strictly increasing");
        }
    }
    Ok(input)
}

#[cfg(test)]
fn format_difference(difference: &TraceDifference) -> String {
    match difference {
        TraceDifference::FrameCrc {
            index,
            left_frame,
            right_frame,
            left_crc,
            right_crc,
        } => format!(
            "trace mismatch at index {index}: left frame {left_frame} crc {left_crc:#010x}, right frame {right_frame} crc {right_crc:#010x}"
        ),
        TraceDifference::Length {
            matching_frames,
            left_len,
            right_len,
        } => format!(
            "trace length mismatch after {matching_frames} matching frames: left has {left_len}, right has {right_len}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use generals_main::deterministic_trace::FrameTrace;

    fn empty_frame(frame: u32) -> FrameTrace {
        FrameTrace::new(frame, [0; 6], Vec::new(), Vec::new(), None)
    }

    #[test]
    fn parser_accepts_wrapped_trace_dump() {
        let json = serde_json::json!({
            "schema": "generals.frame_trace.v2",
            "scenario": "unit",
            "final_frame": 1,
            "frames": [empty_frame(1)]
        });

        let input = parse_trace(&json.to_string()).expect("wrapped trace should parse");
        assert_eq!(input.frames.len(), 1);
        assert_eq!(input.frames[0].frame, 1);
    }

    #[test]
    fn parser_rejects_unversioned_or_empty_traces() {
        assert!(parse_trace(&serde_json::json!([empty_frame(1)]).to_string()).is_err());
        for (schema, frames) in [
            ("generals.frame_trace.v1", vec![empty_frame(1)]),
            ("generals.frame_trace.v2", vec![]),
            ("generals.frame_trace.v2", vec![empty_frame(1), empty_frame(1)]),
        ] {
            let json = serde_json::json!({
                "schema": schema, "scenario": "unit", "frames": frames
            });
            assert!(parse_trace(&json.to_string()).is_err());
        }
    }

    #[test]
    fn parser_rejects_incomplete_v2_frame() {
        let mut frame = serde_json::to_value(empty_frame(1)).unwrap();
        frame.as_object_mut().unwrap().remove("events");
        let json = serde_json::json!({
            "schema": "generals.frame_trace.v2", "scenario": "unit", "frames": [frame]
        });
        assert!(parse_trace(&json.to_string()).is_err());
    }

    #[test]
    fn difference_message_includes_first_divergent_frame() {
        let message = format_difference(&TraceDifference::FrameCrc {
            index: 4,
            left_frame: 100,
            right_frame: 100,
            left_crc: 0x1234,
            right_crc: 0xabcd,
        });

        assert!(message.contains("index 4"));
        assert!(message.contains("frame 100"));
        assert!(message.contains("0x00001234"));
        assert!(message.contains("0x0000abcd"));
    }

    #[test]
    fn field_difference_message_names_category_and_path() {
        let message = format_field_difference(&TraceFieldDifference {
            index: 2,
            left_frame: 3,
            right_frame: 3,
            category: "xfer".to_string(),
            path: "xfer[7]".to_string(),
            left: serde_json::json!(12),
            right: serde_json::json!(13),
        });
        assert!(message.contains("xfer at xfer[7]"));
        assert!(message.contains("left=12"));
        assert!(message.contains("right=13"));
    }
}
