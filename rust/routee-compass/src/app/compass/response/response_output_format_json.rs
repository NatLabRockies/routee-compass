use crate::app::compass::CompassAppError;

pub fn initial_file_contents(newline_delimited: bool) -> Option<String> {
    if newline_delimited {
        None
    } else {
        Some(String::from("[\n"))
    }
}

pub fn final_file_contents(newline_delimited: bool) -> Option<String> {
    if newline_delimited {
        None
    } else {
        Some(String::from("\n]"))
    }
}

pub fn format_response(
    response: &serde_json::Value,
    newline_delimited: bool,
) -> Result<String, CompassAppError> {
    if newline_delimited {
        serde_json::to_string(response).map_err(CompassAppError::from)
    } else {
        let row = serde_json::to_string_pretty(response).map_err(CompassAppError::from)?;
        Ok(row)
    }
}

pub fn delimiter(newline_delimited: bool) -> Option<String> {
    if newline_delimited {
        Some(String::from("\n"))
    } else {
        Some(String::from(",\n"))
    }
}
