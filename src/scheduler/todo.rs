use super::*;

#[derive(Debug, Serialize, Deserialize ,Clone)]
pub enum Todo {
    CreateAttendanceCheck {
        hour: i64,
    },
    SendAttendanceInfo {
        attendance_id: u32,
    },
    Test,
    SendMessage {
        contents: String,
    },
    Nothing,
}

impl Todo {
    pub async fn excute(&self, schedule_id:&str ,time:DateTime<Utc>) {
        match self {
            Self::CreateAttendanceCheck { hour } => {
                create_attendance_check(time + Duration::hours(*hour) ,schedule_id).await;
            }
            Self::Test => {
                println!("called!!!")
            }
            Self::SendAttendanceInfo {
                attendance_id,
            } => {
                let attendance = get_attendance_status(attendance_id).await;
                let attend = attendance.attend.len();
                if attend < 4 {
                    for user_id in attendance.attend.iter().chain(attendance.absent.iter()).chain(attendance.holding.iter()) {
                        let message = line::SimpleMessage::new("今のところ卓が立たなさそうです！！！やばいです！！！");
                        line::push_message(&user_id,message).await;
                    }
                    push_notifications("卓が立たなそうです！！！！", "あわわわわわ。。。。。。",Some(attendance_id.to_string())).await;
                }
            }
            Self::SendMessage {contents} =>{
                push_notifications("今日の練習会は休みです", &format!("理由:{contents}"),None).await;
            }
            Self::Nothing => {}
        }
    }
}