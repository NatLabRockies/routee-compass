use crate::config::CompassConfigurationError;

/// strips the "type" key from the incoming configuration object
pub fn strip_type_from_config(
    config: &serde_json::Value,
) -> Result<(serde_json::Value, String), CompassConfigurationError> {
    let mut conf_clone = config.clone();
    let obj = conf_clone.as_object_mut().ok_or_else(|| {
        let msg = "incoming configuration is not a JSON object type";
        CompassConfigurationError::UserConfigurationError(msg.to_string())
    })?;
    let value = obj.remove("type").ok_or_else(|| {
        let msg = "incoming configuration has no 'type' field";
        CompassConfigurationError::UserConfigurationError(msg.to_string())
    })?;
    let type_str = value.as_str().ok_or_else(|| {
        let msg = "incoming configuration has 'type' field which is not a string";
        CompassConfigurationError::UserConfigurationError(msg.to_string())
    })?;
    Ok((conf_clone, type_str.to_string()))
}
