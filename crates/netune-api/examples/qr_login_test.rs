use netune_api::NeteaseApiClient;
use netune_core::traits::NeteaseClient;

#[tokio::main]
async fn main() {
    let client = NeteaseApiClient::new();

    println!("=== Step 1: Generate QR ===");
    let key = client.login_qr_generate().await.unwrap();
    println!("Key: {key}");
    println!("URL: https://music.163.com/login?codekey={key}");
    println!("\n请用网易云音乐 App 扫描上面的二维码...");
    println!("等待30秒，每2秒轮询一次...\n");

    for i in 1..=15 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        match client.login_qr_check(&key).await {
            Ok(Some(profile)) => {
                println!("✅ 登录成功! uid={} nick={}", profile.uid, profile.nickname);
                return;
            }
            Ok(None) => {
                println!("[{i}] 等待扫码...");
            }
            Err(e) => {
                println!("[{i}] ❌ 错误: {e}");
                return;
            }
        }
    }
    println!("超时");
}
