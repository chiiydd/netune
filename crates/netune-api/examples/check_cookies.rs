//! Debug tool to check what cookies are available from browsers.

fn main() {
    println!("=== Checking browser cookies for music.163.com ===\n");

    // Try chrome
    println!("--- chrome ---");
    let domains = vec!["music.163.com".to_string()];
    match rookie::chrome(Some(domains)) {
        Ok(cookies) => {
            println!("  With filter 'music.163.com': {} cookies", cookies.len());
            for c in &cookies {
                let val_preview = if c.value.len() > 30 {
                    format!("{}...", &c.value[..30])
                } else {
                    c.value.clone()
                };
                println!("    {} (domain={}) = {}", c.name, c.domain, val_preview);
            }
        }
        Err(e) => println!("  Error: {e}"),
    }

    // Try without domain filter
    match rookie::chrome(None) {
        Ok(cookies) => {
            let netease: Vec<_> = cookies.iter().filter(|c| c.domain.contains("163")).collect();
            println!("  All cookies: {} total, {} for 163.com", cookies.len(), netease.len());
            for c in &netease {
                let val_preview = if c.value.len() > 30 {
                    format!("{}...", &c.value[..30])
                } else {
                    c.value.clone()
                };
                println!("    {} (domain={}) = {}", c.name, c.domain, val_preview);
            }
        }
        Err(e) => println!("  Error (no filter): {e}"),
    }

    // Try firefox
    println!("\n--- firefox ---");
    let domains = vec!["music.163.com".to_string()];
    match rookie::firefox(Some(domains)) {
        Ok(cookies) => {
            println!("  With filter 'music.163.com': {} cookies", cookies.len());
            for c in &cookies {
                let val_preview = if c.value.len() > 30 {
                    format!("{}...", &c.value[..30])
                } else {
                    c.value.clone()
                };
                println!("    {} (domain={}) = {}", c.name, c.domain, val_preview);
            }
        }
        Err(e) => println!("  Error: {e}"),
    }

    match rookie::firefox(None) {
        Ok(cookies) => {
            let netease: Vec<_> = cookies.iter().filter(|c| c.domain.contains("163")).collect();
            println!("  All cookies: {} total, {} for 163.com", cookies.len(), netease.len());
            for c in &netease {
                let val_preview = if c.value.len() > 30 {
                    format!("{}...", &c.value[..30])
                } else {
                    c.value.clone()
                };
                println!("    {} (domain={}) = {}", c.name, c.domain, val_preview);
            }
        }
        Err(e) => println!("  Error (no filter): {e}"),
    }
}
