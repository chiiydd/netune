#[tokio::main]
async fn main() {
    let params = serde_json::json!({"type": 1, "noCheckToken": true});
    let (enc_params, enc_sec_key) = netune_api::crypto::weapi_encrypt(&params).unwrap();
    println!("enc_params len: {}", enc_params.len());
    println!("enc_sec_key len: {}", enc_sec_key.len());
    
    let client = reqwest::Client::new();
    let resp = client.post("https://music.163.com/weapi/login/qrcode/unikey")
        .form(&[("params", &enc_params), ("encSecKey", &enc_sec_key)])
        .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64)")
        .header("Referer", "https://music.163.com")
        .send().await.unwrap();
    println!("Status: {}", resp.status());
    let body = resp.text().await.unwrap();
    println!("Body: {}", body);
}
