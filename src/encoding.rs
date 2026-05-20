pub(crate) fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

pub(crate) fn encode_path(value: &str) -> String {
    value
        .split('/')
        .map(encode_path_segment)
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_path_segments() {
        assert_eq!(encode_path_segment("v1.0.0"), "v1.0.0");
        assert_eq!(encode_path_segment("release/one"), "release%2Fone");
        assert_eq!(
            encode_path_segment("name with space"),
            "name%20with%20space"
        );
    }

    #[test]
    fn encodes_path_segments_without_collapsing_slashes() {
        assert_eq!(encode_path("docs/readme#1.md"), "docs/readme%231.md");
        assert_eq!(encode_path("feature//x"), "feature//x");
    }
}
