pub fn sentence_join(items: &Vec<String>) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{} and {}", items[0], items[1]),
        _ => {
            let mut text = String::new();

            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    text.push_str(", ");
                }
                if i == items.len() - 1 {
                    text.push_str("and ");
                }

                text.push_str(&item);
            }

            text
        }
    }
}
