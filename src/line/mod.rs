use super::*;
use serde::Serialize;

pub mod message;
pub use message::*;

pub mod profile;
pub use profile::*;

#[derive(Serialize)]
pub struct BloadcastMessage {
    pub messages: Vec<Box<dyn Message>>,
}
impl BloadcastMessage {
    pub async fn send(&self) {
        let result = send_post_request(
            "https://api.line.me/v2/bot/message/broadcast",
            &serde_json::to_string(self).unwrap(),
        )
        .await;
        if result.is_err() {
            println!("メッセージ送信失敗:{:?}", result.unwrap().text().await);
        }
    }
}

#[derive(Serialize)]
pub struct PushMessage {
    pub to: String,
    pub messages: Vec<Box<dyn Message>>,
}
impl PushMessage {
    pub async fn send(&self) {
        println!("メッセージを送信:{}", serde_json::to_string(self).unwrap());
        let responce = send_post_request(
            "https://api.line.me/v2/bot/message/push",
            &serde_json::to_string(self).unwrap(),
        )
        .await;
        if responce.is_err() {
            println!("メッセージ送信失敗:{:?}", responce);
        }
    }
}

pub async fn push_message(to: &str, message: impl Message) {
    let message = PushMessage {
        to: to.to_string(),
        messages: vec![Box::new(message)],
    };
    message.send().await;
}

pub async fn push_messages(to: &str, messages: Vec<Box<dyn Message>>) {
    let message = PushMessage {
        to: to.to_string(),
        messages,
    };
    message.send().await;
}

async fn send_get_request(url: &str) -> AsyncResult<reqwest::Response> {
    let client = reqwest::Client::new();
    Ok(client
        .get(url)
        .bearer_auth(SETTINGS.TOKEN.to_string())
        .send()
        .await?)
}

async fn send_post_request(url: &str, body: &str) -> AsyncResult<reqwest::Response> {
    let client = reqwest::Client::new();
    Ok(client
        .post(url)
        .header("Content-Type", "application/json")
        .bearer_auth(SETTINGS.TOKEN.to_string())
        .body(body.to_string())
        .send()
        .await?)
}
