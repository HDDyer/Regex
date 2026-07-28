use regex::Regex;

fn main() {
    let regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    let output = regex.is_match("test@test.com");
    println!("{}", output);
}
