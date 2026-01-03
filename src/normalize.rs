use regex::Regex;

lazy_static::lazy_static! {
    static ref RE_CLEAN: Regex = Regex::new(r"[^\p{L}\p{N}\s]").unwrap();
    static ref RE_MULTI_SPACE: Regex = Regex::new(r"\s+").unwrap();
}

pub fn normalize_text(input: &str) -> String {
    let mut s = String::with_capacity(input.len());
    let mut last_was_digit = None; // برای ردیابی نوع کاراکتر قبلی

    for c in input.chars() {
        let current_is_digit = c.is_numeric();
        let current_is_alpha = c.is_alphabetic();

        if let Some(was_digit) = last_was_digit {
            if (was_digit && current_is_alpha) || (!was_digit && current_is_digit) {
                s.push(' ');
            }
        }

        match c {
            'ي' => {
                s.push('ی');
                last_was_digit = Some(false);
            }
            'ك' => {
                s.push('ک');
                last_was_digit = Some(false);
            }
            '\u{200c}' => {
                s.push(' ');
                last_was_digit = None;
            }
            c if c.is_alphanumeric() => {
                for low_c in c.to_lowercase() {
                    s.push(low_c);
                }
                last_was_digit = Some(current_is_digit);
            }
            _ => {
                s.push(' ');
                last_was_digit = None;
            }
        }
    }
    s
}

pub fn tokenize(input: &str) -> Vec<String> {
    normalize_text(input)
        .split_whitespace()
        .map(|w| {
            let mut word = w.to_string();
            if word.len() > 4 {
                if word.ends_with("ها") || word.ends_with("ان") {
                    word.truncate(word.len() - word.chars().last().unwrap().len_utf8() * 2);
                }
            }
            word
        })
        .collect()
}
