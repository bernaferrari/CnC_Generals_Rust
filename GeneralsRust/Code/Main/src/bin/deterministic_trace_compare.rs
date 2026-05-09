use anyhow::{bail, Context, Result};
use generals_main::deterministic_trace::{compare_frame_traces, FrameTrace, TraceDifference};
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
#[serde(untagged)]
enum TraceInput {
    Dump { frames: Vec<FrameTrace> },
    Frames(Vec<FrameTrace>),
}

fn main() -> Result<()> {
    let args = Args::parse(env::args().skip(1))?;
    let left = read_trace(&args.left)?;
    let right = read_trace(&args.right)?;

    match compare_frame_traces(&left, &right) {
        Ok(()) => {
            println!("traces match: {} frames", left.len());
            Ok(())
        }
        Err(difference) => bail!("{}", format_difference(&difference)),
    }
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

fn read_trace(path: &Path) -> Result<Vec<FrameTrace>> {
    let json = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let input: TraceInput =
        serde_json::from_str(&json).with_context(|| format!("parsing {}", path.display()))?;

    Ok(match input {
        TraceInput::Dump { frames } => frames,
        TraceInput::Frames(frames) => frames,
    })
}

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
            "schema": "generalsrust.frame_trace.v1",
            "scenario": "unit",
            "final_frame": 1,
            "frames": [empty_frame(1)]
        });

        let input: TraceInput = serde_json::from_value(json).expect("wrapped trace should parse");
        let TraceInput::Dump { frames } = input else {
            panic!("expected wrapped dump");
        };

        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].frame, 1);
    }

    #[test]
    fn parser_accepts_raw_frame_array() {
        let json = serde_json::json!([empty_frame(1), empty_frame(2)]);

        let input: TraceInput = serde_json::from_value(json).expect("raw trace should parse");
        let TraceInput::Frames(frames) = input else {
            panic!("expected raw frames");
        };

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[1].frame, 2);
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
}
