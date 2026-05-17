pub fn strip_rich_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '[' {
            let mut tag = String::new();
            while let Some(&next) = chars.peek() {
                chars.next();
                if next == ']' {
                    break;
                }
                tag.push(next);
            }

            match tag.as_str() {
                "energy:1" => out.push('E'),
                "star:1" => out.push('S'),
                _ if tag.starts_with("energy:") => out.push('E'),
                _ if tag.starts_with("star:") => out.push('S'),
                _ => {}
            }
        } else {
            out.push(ch);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::strip_rich_text;

    #[test]
    fn strips_style_tags_and_keeps_icon_markers() {
        assert_eq!(
            strip_rich_text("Gain [blue]1[/blue] [gold]Block[/gold] and [energy:1]."),
            "Gain 1 Block and E."
        );
    }
}
