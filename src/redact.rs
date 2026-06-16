use serde_json::Value;

const SECRET_KEYS: &[&str] = &["api_key", "apikey", "auth", "password", "secret", "token"];

pub fn redact_value(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    if SECRET_KEYS
                        .iter()
                        .any(|needle| key.to_ascii_lowercase().contains(needle))
                    {
                        (key, Value::String("[redacted]".to_string()))
                    } else {
                        (key, redact_value(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_value).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn redacts_secret_like_keys_recursively() {
        let value = super::redact_value(json!({
            "token": "abc",
            "nested": { "api_key": "def", "safe": "ok" }
        }));
        assert_eq!(value["token"], "[redacted]");
        assert_eq!(value["nested"]["api_key"], "[redacted]");
        assert_eq!(value["nested"]["safe"], "ok");
    }
}
