//! Test browser cookie import directly.

use netune_api::NeteaseApiClient;
use netune_core::traits::NeteaseClient;

#[tokio::main]
async fn main() {
    let client = NeteaseApiClient::new();

    println!("=== Testing browser cookie import ===\n");

    // Test with auto mode
    println!("Trying auto mode...");
    match client.import_browser_cookies("auto").await {
        Ok(Some(profile)) => {
            println!("✅ Login succeeded! uid={}, nickname={}", profile.uid, profile.nickname);
            println!("   avatar: {:?}", profile.avatar_url);
        }
        Ok(None) => {
            println!("❌ Got None (unexpected)");
        }
        Err(e) => {
            println!("❌ Error: {e}");
        }
    }
}
