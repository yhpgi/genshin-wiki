use scraper::ElementRef;
use serde_json::Value;

#[inline]
pub fn parse_value_as_optional_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub fn format_calendar_date_value(raw_date_val: Option<&Value>) -> Option<String> {
    raw_date_val
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| {
            let parts: Vec<&str> = s.split('-').collect();
            match parts.as_slice() {
                [y, m, d] if y.len() == 4 && m.len() == 2 && d.len() == 2 => {
                    Some(format!("{}-{}", m, d))
                }

                [m, d] if m.len() == 2 && d.len() == 2 => Some(s.to_string()),
                _ => None,
            }
        })
}

pub fn get_alignment_style(element: ElementRef) -> Option<String> {
    element.value().attr("style").and_then(|style| {
        if style.contains("text-align: center") || style.contains("text-align:center") {
            Some("center".to_string())
        } else if style.contains("text-align: right") || style.contains("text-align:right") {
            Some("right".to_string())
        } else if style.contains("text-align: left") || style.contains("text-align:left") {
            Some("left".to_string())
        } else {
            None
        }
    })
}

pub fn get_alignment_attr(element: ElementRef) -> Option<String> {
    element
        .value()
        .attr("align")
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s == "left" || s == "center" || s == "right")
}

pub fn remove_internal_ids_recursive(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("id");

            for (_, val) in map.iter_mut() {
                remove_internal_ids_recursive(val);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                remove_internal_ids_recursive(item);
            }
        }

        _ => {}
    }
}
