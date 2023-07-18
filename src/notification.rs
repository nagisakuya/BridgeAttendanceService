use super::*;


pub async fn push_attendance_notifications(attendance_id: &u32) {
    let quote = get_random_quote().await;

    let row = sqlx::query(&format!(
        "select * from attendance_checks where attendance_id = ?"
    ))
    .bind(attendance_id)
    .fetch_one(DB.get().unwrap())
    .await
    .unwrap();
    let title: String = row.get("description");

    push_notifications(&title, &quote, Some(attendance_id.to_string())).await;
}

pub async fn push_notifications(title: &str, message: &str, attendance_id: Option<String>) {
    use web_push::*;

    let notification_list = sqlx::query("select * from notification")
        .fetch_all(DB.get().unwrap())
        .await
        .unwrap();

    for item in notification_list {
        let user_id: String = item.get("user_id");
        let endpoint: String = item.get("endpoint");
        let key: String = item.get("key");
        let auth: String = item.get("auth");

        let json = serde_json::json! {{
            "title" : title,
            "message": message,
            "attendance_id": attendance_id,
            "user_id": user_id,
        }};
        let content = json.to_string().as_bytes().to_owned();

        match push_notification_at(endpoint, key, auth, &content).await {
            Ok(()) => println!("プッシュ通知を送信しました。userid={}", user_id),
            Err(e) => {
                println!(
                    "プッシュ通知の送信に失敗しました。userid={} err={}",
                    user_id, e
                )
            }
        }
    }

    async fn push_notification_at(
        endpoint: String,
        key: String,
        auth: String,
        content: &[u8],
    ) -> AsyncResult<()> {
        let subscription_info = SubscriptionInfo::new(endpoint, key, auth);

        let private_key = fs::File::open("private_key.pem")?;
        let signature =
            VapidSignatureBuilder::from_pem(private_key, &subscription_info)?.build()?;

        let mut message_builder = WebPushMessageBuilder::new(&subscription_info)?;
        message_builder.set_vapid_signature(signature);

        message_builder.set_payload(ContentEncoding::Aes128Gcm, &content);

        let client = WebPushClient::new()?;
        client.send(message_builder.build()?).await?;
        Ok(())
    }
}

async fn get_random_quote() -> String {
    const DEFAULT_QUOTE: &str = "俺はユース日本一";
    let client = reqwest::Client::new();
    let Ok(resp) = client
        .get("https://meigen.doodlenote.net/api/json.php")
        .query(&[("c", "1")])
        .send()
        .await else{return DEFAULT_QUOTE.to_string()};
    let json = Value::from_str(&resp.text().await.unwrap()).unwrap();
    json.as_array()
        .unwrap()
        .get(0)
        .unwrap()
        .get("meigen")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

pub async fn subscribe(body: Bytes) -> StatusCode {
    let Ok(body) = String::from_utf8(body.to_vec()) else { return StatusCode::BAD_REQUEST };
    println!("SUBSCRIBE:{}", body);
    let Ok(json):Result<Value,_> = serde_json::from_str(&body) else { return StatusCode::BAD_REQUEST };

    let Some(Some(user_id)) = json.get("user_id").map(|i|i.as_str()) else { return StatusCode::BAD_REQUEST };
    let Some(Some(endpoint)) = json.get("endpoint").map(|i|i.as_str()) else { return StatusCode::BAD_REQUEST };
    let Some(Some(key)) = json.get("key").map(|i|i.as_str()) else { return StatusCode::BAD_REQUEST };
    let Some(Some(auth)) = json.get("auth").map(|i|i.as_str()) else { return StatusCode::BAD_REQUEST };

    sqlx::query("replace into notification(user_id,endpoint,key,auth) values(?,?,?,?)")
        .bind(user_id)
        .bind(endpoint)
        .bind(key)
        .bind(auth)
        .execute(DB.get().unwrap())
        .await
        .unwrap();

    StatusCode::OK
}
