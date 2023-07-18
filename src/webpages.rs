use super::*;

pub async fn index(Query(params): Query<HashMap<String, String>>) -> Result<Html<String>, StatusCode> {
    if let Some(user_id) = params.get("user_id") {
        signup(user_id).await;
    };
    let mut html = fs::read_to_string("index.html").unwrap();

    let mut buf = String::new();
    let attendance_checks = sqlx::query("select * from attendance_checks")
        .fetch_all(DB.get().unwrap())
        .await
        .unwrap();

    for item in attendance_checks.iter().rev() {
        let attendance_id: u32 = item.get("attendance_id");
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


pub async fn result_page(
    Query(params): Query<HashMap<String, String>>,
) -> Result<Html<String>, StatusCode> {
    if let Some(user_id) = params.get("user_id") {
        signup(user_id).await;
    };
    let Some(Ok(attendance_id)) = params.get("attendance_id").map(|i|i.parse()) else {return Err(StatusCode::BAD_REQUEST)};
    let attendance = get_attendance_status(&attendance_id);
    let attendance_data = sqlx::query("select * from attendance_checks where attendance_id = ?")
        .bind(&attendance_id)
        .fetch_one(DB.get().unwrap());

    let (attendance, attendance_data) = tokio::join!(attendance, attendance_data);

    let Attendance {
        attend,
        holding,
        absent,
    } = attendance;

    let Ok(attendance_data) = attendance_data else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let title: String = attendance_data.get("description");

    let mut html = fs::read_to_string("result_page.html").unwrap();
    html = html.replace("%TITLE%", &title.to_string());
    html = html.replace("%ATTEND%", &attend.len().to_string());
    html = html.replace("%HOLDING%", &holding.len().to_string());
    html = html.replace("%ABSENT%", &absent.len().to_string());

    async fn ids_to_name(user_ids: &Vec<String>) -> String {
        let mut buf = String::default();
        for user_id in user_ids {
            let (name,picture_url) = match sqlx::query("select * from users where id=?").bind(user_id).fetch_one(DB.get().unwrap()).await {
                Ok(row) => {(row.get("name"),row.get("image"))},
                Err(_) => {
                    let Some(profile) = signup(user_id).await else {
                        continue;
                    };
                    (profile.displayName,profile.pictureUrl.unwrap_or(SETTINGS.DEFAULT_ICON_URL.to_string()))
                },
            };
            buf += {
                let icon =
                    format!(r####"<img src="{picture_url}" alt="icon" class="user_icon">"####);
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