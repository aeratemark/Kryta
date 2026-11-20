//! What every route needs from a request: its form fields, the two command-shaped strings the
//! operator surfaces accept, and the answer helpers a page uses.
//!
//! A route here is a method and a path, and its inputs arrive as an URL-encoded form body. Nothing
//! is fetched beyond the page itself, so a form is a plain `POST` and an answer is another page.

use std::collections::{BTreeMap, HashMap};

use shared::network::http::Response;

/// The fields of a URL-encoded form body, decoded.
pub fn form_fields(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (unescape(key), unescape(value)))
        .collect()
}

/// A field's value, or the empty text: a form that did not state it said nothing.
pub fn value<'a>(form: &'a HashMap<String, String>, name: &str) -> &'a str {
    form.get(name).map(String::as_str).unwrap_or("")
}

/// Decode one URL-encoded field: `+` is a space, and `%XX` is a byte.
pub fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A stack service is an identifier, not a free-form supervisor command. Keeping that boundary here
/// lets the page select only existing protocol verbs and prevents a form value from becoming a
/// second command language.
pub fn service_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

/// The console's argument area uses one `name=value` assignment per line, the same named values the
/// world endpoint accepts. The endpoint still decides whether the names and count are valid for its
/// registration.
pub fn operation_arguments(text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut arguments = BTreeMap::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((name, value)) = line.split_once('=') else {
            return Err(format!("An argument must be name=value: {line}"));
        };
        let name = name.trim();
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(format!(
                "An argument name is letters, digits, or underscore: {name}"
            ));
        }
        if arguments
            .insert(name.to_string(), value.trim().to_string())
            .is_some()
        {
            return Err(format!("An argument is named twice: {name}"));
        }
    }
    Ok(arguments)
}

/// A redirect: a form's answer is another page, and the browser goes there. The answer carries
/// the `Location` header a client follows; the body is only what a client that does not is shown.
pub fn redirect(to: &str) -> Response {
    Response::redirect(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_body_keeps_its_fields() {
        let fields = form_fields("username=a%40b.c&token=x+y");
        assert_eq!(fields.get("username").map(String::as_str), Some("a@b.c"));
        assert_eq!(fields.get("token").map(String::as_str), Some("x y"));
    }

    #[test]
    fn console_arguments_are_named_once_and_left_for_the_world_to_validate() {
        let arguments =
            operation_arguments("destination=ascalon\nmode=hard").expect("arguments parse");
        assert_eq!(arguments.get("destination"), Some(&"ascalon".to_string()));
        assert_eq!(arguments.get("mode"), Some(&"hard".to_string()));
        assert!(operation_arguments("destination=ascalon\ndestination=kryta").is_err());
        assert!(operation_arguments("not an assignment").is_err());
    }

    #[test]
    fn a_service_name_cannot_change_the_supervisor_line() {
        assert!(service_name("world"));
        assert!(service_name("world-2"));
        assert!(!service_name("world restart auth"));
        assert!(!service_name("world\nquit"));
    }
}
