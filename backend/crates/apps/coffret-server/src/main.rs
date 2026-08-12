fn main() {
    println!("{}", greeting());
}

fn greeting() -> &'static str {
    "coffret-server 0.1.0 — nothing implemented yet"
}

#[cfg(test)]
mod tests {
    use super::greeting;

    #[test]
    fn greeting_names_the_binary() {
        assert!(greeting().starts_with("coffret-server"));
    }
}
