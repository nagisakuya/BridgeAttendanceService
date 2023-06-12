use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub enum Todo {
    CreateAttendanceCheck {
        hour: i64,
    },
    SendAttendanceInfo {
        attendance_id: String,
    },
    Test,
    SendMessage {
        contents: line::SimpleMessage,
    },
    Nothing,
}

impl Todo {
    pub async fn excute(&self, schedule_id:&str ,time:DateTime<Utc>) -> Option<Schedule> {
        match self {
            Self::CreateAttendanceCheck { hour } => {
                let schedule =
                    create_attendance_check(time + Duration::hours(*hour) ,schedule_id).await;
                return Some(schedule);
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
                    let message = line::PushMessage {
                        to: SETTINGS.BINDED_GROUP_ID.clone(),
                        messages: vec![Box::new(line::SimpleMessage::new(
                            "今のところ卓が立たなさそうです！！！やばいです！！！",
                        ))],
                    };
                    message.send().await;
                }
            }
            Self::SendMessage {contents} =>{
                let sender = line::PushMessage{
                    to:SETTINGS.BINDED_GROUP_ID.clone(),
                    messages:vec![Box::new(contents.clone())]
                };
                sender.send().await;
            }
            Self::Nothing => {}
        }
        None
    }
}