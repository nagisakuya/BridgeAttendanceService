
use super::*;
use chrono::Weekday;
use serde::{Deserialize, Serialize};

mod schedule;
pub use schedule::*;

mod todo;
pub use todo::*;


#[derive(Debug,Default)]
pub struct Scheduler {
    schedules: Vec<Schedule>,
    timestamp: DateTime<Utc>,
}

impl Scheduler {
    pub async fn from_file(path: &str) -> Self {
        let schedules: Vec<Schedule> =
            serde_json::from_reader(fs::File::open(path).unwrap()).unwrap();
        let timestamp: DateTime<Utc> = sqlx::query("select * from systemdata")
            .fetch_one(DB.get().unwrap())
            .await
            .unwrap()
            .get::<Option<DateTime<Utc>>, _>("timestamp")
            .unwrap_or_else(Utc::now);

        Scheduler {
            schedules,
            timestamp,
        }
    }
    pub async fn save_shedule(&self, path: &str) -> AsyncResult<()> {
        fs::write(path, serde_json::to_string(&self.schedules)?)?;
        Ok(())
    }
    pub async fn check(&mut self) {
        let last = self.timestamp;
        let now = Utc::now();
        let sql_result = sqlx::query("update systemdata set timestamp=?")
            .bind(now)
            .execute(DB.get().unwrap())
            .await;
        if sql_result.is_err() {
            return;
        }
        self.timestamp = now;

        if Schedule::check_schedules(&mut self.schedules, &last, &now).await > 0 {
            self.save_shedule("schedule.json").await.unwrap();
        }
    }
    pub async fn push(&mut self, schedule: Schedule) {
        self.schedules.push(schedule)
    }
    pub fn get(&self, name: &str) -> Option<&Schedule> {
        self.schedules.iter().find(|i| i.id == name)
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Schedule> {
        self.schedules.iter_mut().find(|i| i.id == name)
    }
}

#[tokio::test]
async fn scheduler_gen() {
    let mut scheduler = Scheduler::default();
    let mon = ScheduleType::weekly(Weekday::Mon, NaiveTime::from_hms_opt(10, 0, 0).unwrap());
    let thu = ScheduleType::weekly(Weekday::Thu, NaiveTime::from_hms_opt(10, 0, 0).unwrap());
    scheduler
        .push(Schedule {
            id: "".to_string(),
            schedule_type: mon,
            todo: Todo::CreateAttendanceCheck {
                hour: 6,
            },
        })
        .await;
    scheduler
        .push(Schedule {
            id: "".to_string(),
            schedule_type: thu,
            todo: Todo::CreateAttendanceCheck {
                hour: 6,
            },
        })
        .await;
    scheduler.save_shedule("schedule.json").await.unwrap();
}

#[tokio::test]
async fn scheduler_test() {
    let mut scheduler = Scheduler::default();
    let _weekday = ScheduleType::Weekly {
        weekday: Weekday::Mon,
        time: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
        exception: vec![],
    };
    let _onetime = ScheduleType::OneTime {
        datetime: Utc::now() + Duration::seconds(10),
    };
    scheduler
        .push(Schedule {
            id: "".to_string(),
            schedule_type: _onetime,
            todo: Todo::Test,
        })
        .await;
    let shedule_check = async {
        loop {
            scheduler.check().await;
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    };
    shedule_check.await;
}

#[tokio::test]
async fn serde_test() {
    let mut scheduler = Scheduler::default();
    let schedule = Schedule {
        id: "".to_string(),
        schedule_type: ScheduleType::OneTime {
            datetime: Utc::now(),
        },
        todo: Todo::CreateAttendanceCheck {
            hour: 7,
        },
    };
    scheduler.schedules.push(schedule);
    scheduler.save_shedule("schedule.json").await.unwrap();
}
