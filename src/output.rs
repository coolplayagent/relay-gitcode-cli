use serde::Serialize;
use serde_json::Value;

pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn print_value(json: bool, value: &Value) -> anyhow::Result<()> {
    if json {
        print_json(value)
    } else {
        println!("{}", text_summary(value));
        Ok(())
    }
}

pub fn text_summary(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            if let Some(html_url) = object
                .get("html_url")
                .or_else(|| object.get("web_url"))
                .or_else(|| object.get("url"))
                .and_then(Value::as_str)
            {
                return html_url.to_string();
            }
            if let Some(name) = object
                .get("full_name")
                .or_else(|| object.get("path_with_namespace"))
                .or_else(|| object.get("name"))
                .or_else(|| object.get("title"))
                .and_then(Value::as_str)
            {
                return name.to_string();
            }
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
        Value::Array(items) => items.iter().map(row_summary).collect::<Vec<_>>().join("\n"),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn row_summary(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let mut fields = Vec::new();
            for key in [
                "number",
                "id",
                "full_name",
                "name",
                "title",
                "state",
                "html_url",
            ] {
                if let Some(value) = object.get(key).and_then(simple_value) {
                    fields.push(value);
                }
            }
            if fields.is_empty() {
                serde_json::to_string(value).unwrap_or_else(|_| value.to_string())
            } else {
                fields.join("\t")
            }
        }
        _ => value.to_string(),
    }
}

fn simple_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn summarizes_arrays_as_rows() {
        let value = json!([
            {"number": 1, "title": "first", "state": "open"},
            {"number": 2, "title": "second", "state": "closed"}
        ]);
        assert_eq!(text_summary(&value), "1\tfirst\topen\n2\tsecond\tclosed");
    }

    #[test]
    fn summarizes_url_objects_by_url() {
        let value = json!({"html_url": "https://gitcode.com/a/b", "name": "b"});
        assert_eq!(text_summary(&value), "https://gitcode.com/a/b");
    }
}
