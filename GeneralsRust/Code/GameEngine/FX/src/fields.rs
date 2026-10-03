use crate::definition::*;
use std::collections::HashMap;
pub(super) fn property<'a>(properties: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    properties
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

pub(super) fn real(
    properties: &HashMap<String, String>,
    name: &str,
    default: f32,
) -> FXListResult<f32> {
    let Some(value) = property(properties, name) else {
        return Ok(default);
    };
    value
        .parse::<f32>()
        .map_err(|_| FXListError::ParseError(format!("invalid {} value '{}'", name, value)))
}

pub(super) fn integer(
    properties: &HashMap<String, String>,
    name: &str,
    default: i32,
) -> FXListResult<i32> {
    let Some(value) = property(properties, name) else {
        return Ok(default);
    };
    value
        .parse::<i32>()
        .map_err(|_| FXListError::ParseError(format!("invalid {} value '{}'", name, value)))
}

pub(super) fn boolean(
    properties: &HashMap<String, String>,
    name: &str,
    default: bool,
) -> FXListResult<bool> {
    let Some(value) = property(properties, name) else {
        return Ok(default);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Ok(true),
        "no" | "false" | "0" => Ok(false),
        _ => Err(FXListError::ParseError(format!(
            "invalid {} boolean '{}'",
            name, value
        ))),
    }
}

pub(super) fn labelled_vec3(value: &str) -> FXListResult<(f32, f32, f32)> {
    let mut result = [None; 3];
    for component in value.split_whitespace() {
        if let Some((label, raw)) = component.split_once(':') {
            // Retail FXList.ini contains `Y:15:` in two offsets; C++ `atof`
            // accepts the numeric prefix, so tolerate that trailing colon.
            let parsed = raw.trim_end_matches(':').parse::<f32>().map_err(|_| {
                FXListError::ParseError(format!("invalid vector component '{}'", component))
            })?;
            match label.to_ascii_uppercase().as_str() {
                "X" | "R" => result[0] = Some(parsed),
                "Y" | "G" => result[1] = Some(parsed),
                "Z" | "B" => result[2] = Some(parsed),
                _ => {
                    return Err(FXListError::ParseError(format!(
                        "unknown vector component '{}'",
                        label
                    )));
                }
            }
        }
    }
    Ok((
        result[0].ok_or_else(|| FXListError::ParseError("missing X/R component".into()))?,
        result[1].ok_or_else(|| FXListError::ParseError("missing Y/G component".into()))?,
        result[2].ok_or_else(|| FXListError::ParseError("missing Z/B component".into()))?,
    ))
}

pub(super) fn color(value: &str) -> FXListResult<(f32, f32, f32)> {
    let (r, g, b) = labelled_vec3(value)?;
    if [r, g, b]
        .iter()
        .any(|v| !(0.0..=255.0).contains(v) || v.fract() != 0.0)
    {
        return Err(FXListError::ParseError(
            "RGB components must be integers from 0 to 255".into(),
        ));
    }
    Ok((r / 255.0, g / 255.0, b / 255.0))
}
pub(super) fn random_variable(
    properties: &HashMap<String, String>,
    name: &str,
    default: f32,
) -> FXListResult<FxRandomVariable> {
    let Some(value) = property(properties, name) else {
        return Ok(default.into());
    };
    let tokens: Vec<_> = value.split_whitespace().collect();
    let parse = |index: usize| {
        tokens
            .get(index)
            .and_then(|v| v.parse::<f32>().ok())
            .ok_or_else(|| FXListError::ParseError(format!("invalid {} range '{}'", name, value)))
    };
    let distribution = match tokens
        .get(2)
        .copied()
        .unwrap_or("UNIFORM")
        .to_ascii_uppercase()
        .as_str()
    {
        "CONSTANT" => Distribution::Constant,
        "UNIFORM" => Distribution::Uniform,
        "GAUSSIAN" => Distribution::Gaussian,
        "TRIANGULAR" => Distribution::Triangular,
        "LOW_BIAS" => Distribution::LowBias,
        "HIGH_BIAS" => Distribution::HighBias,
        other => {
            return Err(FXListError::ParseError(format!(
                "unknown distribution '{}'",
                other
            )));
        }
    };
    Ok(FxRandomVariable {
        minimum: parse(0)?,
        maximum: parse(1)?,
        distribution,
    })
}
