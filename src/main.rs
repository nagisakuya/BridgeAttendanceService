use axum::body::Bytes;
use axum::extract::Query;
use axum::response::Html;
use axum::routing::get_service;
use axum::*;
use axum_server::tls_rustls::*;
use chrono::{prelude::*, Duration, FixedOffset};
use line::{FlexMessage, SimpleMessage};
use once_cell::sync::{Lazy, OnceCell};
use reqwest::StatusCode;
use serde_json::Value;
use sqlx::{Row, Sqlite};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::{fs, path::PathBuf};
use tokio::sync::Mutex;

pub mod line;

pub mod scheduler;
pub use scheduler::*;

#[allow(non_snake_case)]
#[derive(serde::Deserialize)]
struct Settings {
    TOKEN: String,
    TLS_KEY_DIR_PATH: PathBuf,
    HOST: String,
    LISTENING_ADDRESS: String,
    DEFAULT_ICON_URL: String,
}

static SETTINGS: Lazy<Settings> =
    Lazy::new(|| toml::from_str(&fs::read_to_string("settings.toml").unwrap()).unwrap());

static DB: OnceCell<sqlx::pool::Pool<Sqlite>> = OnceCell::new();
async fn initialize_db() {
    DB.set(sqlx::SqlitePool::connect("database.sqlite").await.unwrap())
        .unwrap();
}

static TIMEZONE: Lazy<FixedOffset> = Lazy::new(|| FixedOffset::east_opt(9 * 3600).unwrap());

static SCHEDULER: OnceCell<Mutex<Scheduler>> = OnceCell::new();
async fn initialize_scheduler() {
    SCHEDULER
        .set(Mutex::new(Scheduler::from_file("schedule.json").await))
        .unwrap();
}

type AsyncResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AsyncResult<()> {
    initialize_db().await;
    initialize_scheduler().await;

    let root = get_service(tower_http::services::ServeDir::new("root")).handle_error(
        |error: std::io::Error| async move {
            (StatusCode::NOT_FOUND, format!("file not found: {}", error))
        },
    );

    let app = Router::new()
        .route("/index", routing::get(index))
        .route("/line/webhook", routing::post(resieve_webhook))
        .route("/result", routing::get(result_page))
        .route("/register", routing::post(register))
        .route("/subscribe", routing::post(subscribe))
        .nest_service("/", root);

    let rustls_config = RustlsConfig::from_pem_file(
        SETTINGS.TLS_KEY_DIR_PATH.join("fullchain.pem"),
        SETTINGS.TLS_KEY_DIR_PATH.join("privkey.pem"),
    )
    .await
    .unwrap();

    let addr = SocketAddr::from_str(&SETTINGS.LISTENING_ADDRESS).unwrap();
    let excute_https_server =
        axum_server::bind_rustls(addr, rustls_config).serve(app.clone().into_make_service());

    let shedule_check = async {
        loop {
            let schedules = SCHEDULER.get().unwrap().lock().await.get_schedules().await;
            for (schedule,fired_time) in schedules{
                println!("イベント発火:{:?}",schedule);
                schedule.todo.excute(&schedule.id, fired_time).await;
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    };

    let (result, _) = tokio::join!(excute_https_server, shedule_check);
    result?;

    Ok(())
}

async fn signup(user_id: &str) {
    let Some(profile) = line::get_user_profile_from_friend(user_id.to_string()).await else {
        return;
    };

    sqlx::query(&format!("replace into users(id,name,image) values(?,?,?)"))
        .bind(&profile.userId)
        .bind(profile.displayName)
        .bind(
            profile
                .pictureUrl
                .as_ref()
                .unwrap_or(&SETTINGS.DEFAULT_ICON_URL),
        )
        .execute(DB.get().unwrap())
        .await
        .unwrap();
}

async fn index(Query(params): Query<HashMap<String, String>>) -> Result<Html<String>, StatusCode> {
    if let Some(user_id) = params.get("user_id") {
        signup(user_id).await
    };
    let mut html = fs::read_to_string("index.html").unwrap();

    let mut buf = String::new();
    let attendance_checks = sqlx::query("select * from attendances")
        .fetch_all(DB.get().unwrap())
        .await
        .unwrap();

    for item in attendance_checks.iter().rev() {
        let attendance_id: String = item.get("attendance_id");
        let description: String = item.get("description");

        let line = format!(
            r#"<button class="link" onclick="location.href='result?attendance_id={}'">{}</div><br>"#,
            attendance_id, description
        );

        buf += &line;
    }

    html = html.replace("%ATTENDANCE_CHECKS%", &buf);
    Ok(Html::from(html))
}

async fn register(body: Bytes) -> StatusCode {
    let Ok(body) = String::from_utf8(body.to_vec()) else { return StatusCode::BAD_REQUEST };
    println!("REGISTRATION:{}", body);
    let Ok(json):Result<Value,_> = serde_json::from_str(&body) else { return StatusCode::BAD_REQUEST };

    let Some(Some(attendance_id)) = json.get("attendance_id").map(|i|i.as_str()) else { return StatusCode::BAD_REQUEST };
    let Some(Some(user_id)) = json.get("user_id").map(|i|i.as_str()) else { return StatusCode::BAD_REQUEST };
    let Some(Some(request_type)) = json.get("request_type").map(|i|i.as_str()) else { return StatusCode::BAD_REQUEST };

    if request_type != "attend" && request_type != "holding" && request_type != "absent" {
        return StatusCode::BAD_REQUEST;
    }

    if let Err(_) = sqlx::query(&format!(
        "replace into {attendance_id} (user_id, status) values (?, '{request_type}')",
    ))
    .bind(user_id)
    .execute(DB.get().unwrap())
    .await
    {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };

    StatusCode::OK
}

async fn resieve_webhook(body: Bytes) -> StatusCode {
    let body = match String::from_utf8(body.to_vec()) {
        Ok(x) => x,
        Err(_) => return StatusCode::BAD_REQUEST,
    };
    let json: Value = match serde_json::from_str(&body) {
        Ok(x) => x,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let Some(events) = json.get("events") else { return StatusCode::BAD_REQUEST };
    let Some(events) = events.as_array() else { return StatusCode::BAD_REQUEST };

    for event in events {
        let event_type = event.get("type").map(|f| f.as_str().unwrap_or_default());
        match event_type {
            Some("message") => {
                resieve_message(event).await;
            }
            Some("follow") => {
                println!("FOLLOW:{}", body);
                let user_id = event
                    .get("source")
                    .unwrap()
                    .get("userId")
                    .unwrap()
                    .as_str()
                    .unwrap();
                send_follow_messages(&user_id).await;
            }
            _ => (),
        }
    }

    StatusCode::OK
}

async fn send_follow_messages(user_id: &str) {
    let signup_url = format!(
        r"https://{}/index?user_id={}&openExternalBrowser=1",
        SETTINGS.HOST, user_id
    );

    let first_message = SimpleMessage::new(
        "友達登録ありがとうございます😊\n下のボタンから出欠システムに登録できます！",
    );

    let mut flex = fs::read_to_string("button.json").unwrap();
    flex = flex.replace("%SIGNUP_URL%", &signup_url);
    let second_message = FlexMessage::new(serde_json::from_str(&flex).unwrap(), "flexメッセージ");

    let third_message = SimpleMessage::new("iosで通知機能を使うためには、ホーム画面にアイコンを追加してね。やり方→https://blog.thetheorier.com/entry/ios16-pwa#:~:text=%E8%A8%AD%E5%AE%9A2");

    line::push_messages(
        user_id,
        vec![
            Box::new(first_message),
            Box::new(second_message),
            Box::new(third_message),
        ],
    )
    .await;
}

async fn resieve_message(event: &Value) -> Option<()> {
    let message: &Value = event.get("message")?;
    if message.get("type")? != "text" {
        return None;
    }
    //let reply_token = event.get("replyToken")?.as_str()?;
    let author = event.get("source")?.get("userId")?.as_str()?;
    let from = event.get("source")?.get("type")?.as_str()?;
    if from != "user" { return None }

    let text = message.get("text")?.as_str()?.to_string();
    let lines: Vec<&str> = text.lines().collect();
    println!("メッセージを受信:{}", event);
    
    let text = match *lines.first()? {
        "休み登録" => push_exception(lines).await.get(),
        "イベント登録" => push_event(lines).await.get(),
        "使い方" => fs::read_to_string("usage.txt").unwrap(),
        "フォロー" => {
            send_follow_messages(author).await;
            return None;
        }
        _ => {
            return None;
        }
    };
    line::push_message(author, line::SimpleMessage::new(&text)).await;
    Some(())
}

enum LineResponse {
    Success(String),
    DateParseError,
    NotEnoughArgment,
    PassedDate,
    UnvalidDate,
    EventNotFound,
}
impl LineResponse {
    fn get(self) -> String {
        match self {
            LineResponse::Success(s) => s,
            LineResponse::DateParseError => "日付の形式が違います".to_owned(),
            LineResponse::NotEnoughArgment => "パラメータが足りません".to_owned(),
            LineResponse::PassedDate => "過去の日付です".to_owned(),
            LineResponse::UnvalidDate => "不正な日付です".to_owned(),
            LineResponse::EventNotFound => "イベントが見つかりません".to_owned(),
        }
    }
}

async fn push_exception(args: Vec<&str>) -> LineResponse {
    let Some(&name) = args.get(1) else {return LineResponse::NotEnoughArgment};
    let Some(&date) = args.get(2) else {return LineResponse::NotEnoughArgment};
    let Ok(date) = NaiveDate::parse_from_str(date,"%Y/%m/%d")else {return LineResponse::DateParseError};
    let reason = args.get(3);
    let mut scheduler = SCHEDULER.get().unwrap().lock().await;
    let Some(&mut Schedule {
        schedule_type:
            ScheduleType::Weekly {
                weekday,
                time,
                ref mut exception,
            },
        ..
    }) = scheduler.get_mut(name) else {return LineResponse::EventNotFound};

    if weekday != date.weekday() {
        return LineResponse::UnvalidDate;
    }
    let datetime = {
        let local = NaiveDateTime::new(date, time)
            .and_local_timezone(*TIMEZONE)
            .unwrap();
        DateTime::<Utc>::from_utc(local.naive_utc(), Utc)
    };
    if datetime < Utc::now() {
        return LineResponse::PassedDate;
    }
    let todo = match reason {
        Some(o) => Todo::SendMessage {
            contents: o.to_string(),
        },
        None => Todo::Nothing,
    };
    let temp = Schedule {
        id: "休み".to_string(),
        schedule_type: ScheduleType::OneTime { datetime },
        todo,
    };
    exception.push(temp);

    scheduler.save_shedule("schedule.json").await.unwrap();
    LineResponse::Success("休み登録成功".to_owned())
}

async fn push_event(args: Vec<&str>) -> LineResponse {
    let Some(&name) = args.get(1) else {return LineResponse::NotEnoughArgment};
    let Some(&date) = args.get(2) else {return LineResponse::NotEnoughArgment};
    let duration_hour: Option<i64> = args.get(3).map(|x| x.parse().ok()).unwrap_or_default();
    let Ok(date) = NaiveDateTime::parse_from_str(date,"%Y/%m/%d %H:%M") else {return LineResponse::DateParseError};
    let date = date.and_local_timezone(*TIMEZONE).unwrap();

    if let Some(hour) = duration_hour {
        let schedule = Schedule {
            id: name.to_string(),
            schedule_type: ScheduleType::OneTime {
                datetime: {
                    let send = date - Duration::hours(hour);
                    if send < Utc::now() {
                        return LineResponse::PassedDate;
                    }
                    DateTime::<Utc>::from_utc(send.naive_utc(), Utc)
                },
            },
            todo: Todo::CreateAttendanceCheck { hour: hour },
        };
        let mut scheduler = SCHEDULER.get().unwrap().lock().await;
        scheduler.push(schedule).await;
        LineResponse::Success("イベントの登録に成功しました".to_string())
    } else {
        if date < Utc::now() {
            return LineResponse::PassedDate;
        }
        create_attendance_check(DateTime::<Utc>::from_utc(date.naive_utc(), Utc), name).await;
        LineResponse::Success("イベントを送信しました".to_string())
    }
}

struct Attendance {
    attend: Vec<String>,
    holding: Vec<String>,
    absent: Vec<String>,
}
async fn get_attendance_status(attendance_id: &str) -> AsyncResult<Attendance> {
    let query = &format!("select * from {attendance_id} where status = ?");
    let attend: Vec<String> = sqlx::query_scalar(query)
        .bind("attend")
        .fetch_all(DB.get().unwrap())
        .await?;
    let holding: Vec<String> = sqlx::query_scalar(query)
        .bind("holding")
        .fetch_all(DB.get().unwrap())
        .await?;
    let absent: Vec<String> = sqlx::query_scalar(query)
        .bind("absent")
        .fetch_all(DB.get().unwrap())
        .await?;
    Ok(Attendance {
        attend,
        holding,
        absent,
    })
}

async fn result_page(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Html<String>, StatusCode> {
    if let Some(user_id) = params.get("user_id") {
        signup(user_id).await
    };
    let Some(attendance_id) = params.get("attendance_id") else {return Err(StatusCode::BAD_REQUEST)};
    let attendance = get_attendance_status(&attendance_id);
    let attendance_data = sqlx::query("select * from attendances where attendance_id = ?")
        .bind(&attendance_id)
        .fetch_one(DB.get().unwrap());

    let (attendance, attendance_data) = tokio::join!(attendance, attendance_data);

    let Ok(Attendance {
        attend,
        holding,
        absent,
    }) = attendance else{
        return Err(StatusCode::BAD_REQUEST)
    };

    let attendance_data = attendance_data.unwrap();

    let title: String = attendance_data.get("description");

    let mut html = fs::read_to_string("result_page.html").unwrap();
    html = html.replace("%TITLE%", &title.to_string());
    html = html.replace("%ATTEND%", &attend.len().to_string());
    html = html.replace("%HOLDING%", &holding.len().to_string());
    html = html.replace("%ABSENT%", &absent.len().to_string());

    async fn ids_to_name(user_ids: &Vec<String>) -> String {
        let mut buf = String::default();
        for user_id in user_ids {
            let Ok(row) = sqlx::query("select * from users where id=?").bind(user_id).fetch_one(DB.get().unwrap()).await else {continue;};
            let name: String = row.get("name");
            let picture_url: String = row.get("image");
            buf += {
                let icon = format!(r####"<img src="{picture_url}" alt="icon" class="user_icon">"####);
                &format!(r##"<div class="box">{}{}</div><br>"##, icon, name)
            };
        }
        buf
    }

    let attends = ids_to_name(&attend);
    let holdings = ids_to_name(&holding);
    let absents = ids_to_name(&absent);

    let (attends, holdings, absents) = tokio::join!(attends, holdings, absents);
    html = html.replace("%ATTENDS%", &attends);
    html = html.replace("%HOLDINGS%", &holdings);
    html = html.replace("%ABSENTS%", &absents);

    Ok(Html::from(html))
}

async fn create_attendance_check(finishing_time: DateTime<Utc>, event_name: &str) {
    //ランダムid生成
    use rand::Rng;
    let attendance_id = "attendance".to_owned() + &rand::thread_rng().gen::<u64>().to_string();

    let text = format!(
        "{}/{}({}){}",
        finishing_time.month(),
        finishing_time.day(),
        weekday_to_jp(finishing_time.weekday()),
        event_name
    );

    //sqlに登録
    sqlx::query(
        "insert into attendances(description,finishing_schedule,attendance_id) values(?,?,?)",
    )
    .bind(&text)
    .bind(finishing_time)
    .bind(&attendance_id)
    .execute(DB.get().expect("DBの取得に失敗しました"))
    .await
    .expect("attendancesテーブルへの書き込みに失敗しました");

    //出欠管理用のテーブル作成
    sqlx::query(&format!(
        "create table {attendance_id}(user_id string primary key,status string)"
    ))
    .execute(DB.get().expect("DBの取得に失敗しました"))
    .await
    .expect("attendanceテーブルの作成に失敗しました");

    let schedule = Schedule {
        id: "".to_string(),
        schedule_type: ScheduleType::OneTime {
            datetime: finishing_time,
        },
        todo: Todo::SendAttendanceInfo { attendance_id:attendance_id.clone() },
    };
    SCHEDULER.get().unwrap().lock().await.push(schedule).await;

    //通知を送信
    push_attendance_notifications(&attendance_id).await;
}

async fn push_attendance_notifications(attendance_id: &str) {
    let quote = get_random_quote().await;

    let row = sqlx::query("select * from attendances where attendance_id = ?")
        .bind(&attendance_id)
        .fetch_one(DB.get().unwrap())
        .await
        .unwrap();
    let title: String = row.get("description");

    push_notifications(&title, &quote, Some(attendance_id.to_string())).await;
}

async fn push_notifications(title: &str, message: &str, attendance_id: Option<String>) {
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

        match push_notification_at(endpoint,key,auth,&content).await {
            Ok(()) => (),
            Err(e) => {println!("プッシュ通知の送信に失敗しました。userid={} err={}",user_id,e)},
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

async fn subscribe(body: Bytes) -> StatusCode {
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

fn weekday_to_jp(weekday: chrono::Weekday) -> String {
    match weekday {
        Weekday::Sun => "日".to_string(),
        Weekday::Mon => "月".to_string(),
        Weekday::Tue => "火".to_string(),
        Weekday::Wed => "水".to_string(),
        Weekday::Thu => "木".to_string(),
        Weekday::Fri => "金".to_string(),
        Weekday::Sat => "土".to_string(),
    }
}
