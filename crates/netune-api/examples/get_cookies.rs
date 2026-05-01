//! Get full cookie values for testing.

fn main() {
    let domains = vec!["music.163.com".to_string()];
    match rookie::firefox(Some(domains)) {
        Ok(cookies) => {
            for c in &cookies {
                if c.name == "MUSIC_U" || c.name == "__csrf" {
                    println!("{}={}", c.name, c.value);
                }
            }
        }
        Err(e) => println!("Error: {e}"),
    }
}
