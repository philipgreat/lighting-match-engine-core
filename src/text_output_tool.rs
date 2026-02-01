
pub fn print_separator(eq_len: usize) {
    println!("\n{}\n", "=".repeat(eq_len));
}

pub fn print_centered_line(text: &str, fill: char, total_width: usize) {
    let text_len = text.len();

    if total_width <= text_len {
        println!("{}", text);
        return;
    }

    let padding = total_width - text_len;
    let left = padding / 2;
    let right = padding - left;

    println!(
        "{}{}{}",
        fill.to_string().repeat(left),
        text,
        fill.to_string().repeat(right)
    );
}