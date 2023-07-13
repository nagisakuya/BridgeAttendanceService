use super::*;

#[derive(Debug, Serialize, Deserialize ,Clone)]
pub enum ScheduleType {
    OneTime {
        datetime: DateTime<Utc>,
    },
    Weekly {
        weekday: Weekday,
        time: NaiveTime,
        exception: Vec<Schedule>,
    },
}
impl ScheduleType {
    pub fn weekly(weekday: Weekday, time: NaiveTime) -> Self {
        Self::Weekly {
            weekday,
            time,
            exception: vec![],
        }
    }
    pub fn check(&self, last: &DateTime<Utc>, now: &DateTime<Utc>) -> (bool, DateTime<Utc>) {
        match self {
            Self::OneTime { datetime } => (last < datetime && datetime <= now, *datetime),
            Self::Weekly { weekday, time, .. } => {
                //get latest date where certain weekday and time
                let mut temp = weekday.num_days_from_monday() as i64
                    - now.weekday().num_days_from_monday() as i64;
                if temp > 0 {
                    temp -= 7
                }
                let target_day = *now + Duration::days(temp);
                let local_datetime = NaiveDateTime::new(target_day.date_naive(), *time)
                    .and_local_timezone(*TIMEZONE)
                    .unwrap();
                let target_datetime: DateTime<Utc> =
                    DateTime::from_utc(local_datetime.naive_utc(), Utc);
                //and compare
                (
                    last < &target_datetime && &target_datetime <= now,
                    target_datetime,
                )
            }
        }
    }
    fn delete_check(&self) -> bool {
        match self {
            Self::OneTime { .. } => true,
            Self::Weekly { .. } => false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Schedule {
    pub id: String,
    pub todo: Todo,
    pub schedule_type: ScheduleType,
}

impl Schedule {
    //returns is it excuted
    #[async_recursion::async_recursion]
    async fn check(&mut self, last: &DateTime<Utc>, now: &DateTime<Utc>) -> Option<(Schedule,DateTime<Utc>)> {
        if let ScheduleType::Weekly {
            ref mut exception, ..
        } = self.schedule_type
        {
            for item in exception {
                if let Some(exception) = item.check(last, now).await {
                    return Some(exception.clone());
                }
            }
        }
        let (fired, fired_time) = self.schedule_type.check(last, now);
        if fired {
            return Some((self.clone(),fired_time));
        }
        return None;
    }
    pub async fn check_schedules(
        schedules: &mut Vec<Schedule>,
        last: &DateTime<Utc>,
        now: &DateTime<Utc>,
    ) -> Vec<(Schedule,DateTime<Utc>)> {
        let mut index = 0;
        let mut rst = vec![];
        while index < schedules.len() {
            let item = schedules.get_mut(index).unwrap();
            if let Some(s) = item.check(last, now).await {
                rst.push(s);
                if item.schedule_type.delete_check() {
                    schedules.remove(index);
                    continue;
                }
            }
            index += 1;
        }
        rst
    }
}
