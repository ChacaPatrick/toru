pub(super) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

/// The text up to the first NUL.
pub(super) fn until_nul(text: &[u16]) -> String {
    let length = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    String::from_utf16_lossy(&text[..length])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_read_up_to_its_nul() {
        let text: Vec<u16> = "abc\0def".encode_utf16().collect();
        assert_eq!(until_nul(&text), "abc");
        assert_eq!(until_nul(&[]), "");
        assert_eq!(until_nul(&[0]), "");
    }
}
