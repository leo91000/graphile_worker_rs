use super::*;
use anyhow::Result;
use chrono::Weekday;

#[test]
fn crontab_timer_should_run_at() -> Result<()> {
    let crontab_timer = CrontabTimer {
        minutes: vec![CrontabValue::Number(30)],
        hours: vec![CrontabValue::Range(8, 10)],
        days: vec![CrontabValue::Step(4)],
        ..Default::default()
    };

    assert!(crontab_timer.should_run_at(&"2012-12-17T08:30:12".parse()?));
    assert!(crontab_timer.should_run_at(&"2015-02-05T09:30:00".parse()?));
    assert!(crontab_timer.should_run_at(&"1998-10-13T10:30:59".parse()?));

    assert!(!crontab_timer.should_run_at(&"2012-12-17T11:30:59".parse()?));
    assert!(!crontab_timer.should_run_at(&"2015-02-05T09:31:00".parse()?));
    assert!(!crontab_timer.should_run_at(&"2012-12-13T08:29:12".parse()?));
    assert!(!crontab_timer.should_run_at(&"1998-10-04T10:30:59".parse()?));

    Ok(())
}

#[test]
fn crontab_fill_ordering() {
    let biggest_fill = CrontabFill::new(1, 30, 0, 0, 0);
    let bigger_fill = CrontabFill::new(0, 32, 0, 0, 0);
    let lower_fill = CrontabFill::new(0, 0, 432, 0, 0);
    let lowest_fill = CrontabFill::new(2, 0, 0, 0, 0);

    let mut fills = vec![&lower_fill, &bigger_fill, &biggest_fill, &lowest_fill];
    fills.sort();
    assert_eq!(
        vec![&lowest_fill, &lower_fill, &bigger_fill, &biggest_fill],
        fills
    );
}

#[test]
fn crontab_fill_to_secs() {
    let fill = CrontabFill::new(0, 0, 0, 0, 0);
    assert_eq!(0, fill.to_secs());
    let fill = CrontabFill::new(0, 0, 0, 0, 1);
    assert_eq!(1, fill.to_secs());
    let fill = CrontabFill::new(1, 30, 28, 350, 2);
    assert_eq!(3318602, fill.to_secs());
}

#[test]
fn crontab_fill_constructors() {
    assert_eq!(CrontabFill::seconds(2).to_secs(), 2);
    assert_eq!(CrontabFill::minutes(2).to_secs(), 120);
    assert_eq!(CrontabFill::hours(2).to_secs(), 7200);
    assert_eq!(CrontabFill::days(2).to_secs(), 172800);
    assert_eq!(CrontabFill::weeks(2).to_secs(), 1209600);
}

#[test]
fn crontab_timer_constructors_build_expected_values() -> Result<()> {
    assert_eq!(CrontabTimer::every_minute(), CrontabTimer::default());

    assert_eq!(
        CrontabTimer::every_n_minutes(5)?.minutes,
        vec![CrontabValue::Step(5)]
    );

    let daily = CrontabTimer::daily_at(8, 30)?;
    assert_eq!(daily.hours, vec![CrontabValue::Number(8)]);
    assert_eq!(daily.minutes, vec![CrontabValue::Number(30)]);

    let weekly = CrontabTimer::weekly_on(Weekday::Mon, 8, 30)?;
    assert_eq!(weekly.dows, vec![CrontabValue::Number(1)]);

    let monthly = CrontabTimer::monthly_on(15, 8, 30)?;
    assert_eq!(monthly.days, vec![CrontabValue::Number(15)]);

    let yearly = CrontabTimer::yearly_on(12, 15, 8, 30)?;
    assert_eq!(yearly.months, vec![CrontabValue::Number(12)]);

    Ok(())
}

#[test]
fn crontab_timer_constructors_validate_values() {
    assert_eq!(
        CrontabTimer::daily_at(24, 0),
        Err(CrontabTimerError::ValueOutOfRange {
            field: CrontabField::Hour,
            value: 24,
            min: 0,
            max: 23,
        })
    );
    assert_eq!(
        CrontabTimer::every_n_minutes(0),
        Err(CrontabTimerError::ZeroStep {
            field: CrontabField::Minute
        })
    );
    assert_eq!(
        CrontabTimer::every_n_minutes(60),
        Err(CrontabTimerError::ValueOutOfRange {
            field: CrontabField::Minute,
            value: 60,
            min: 1,
            max: 59,
        })
    );
    assert_eq!(
        CrontabValue::range(CrontabField::Day, 31, 1),
        Err(CrontabTimerError::RangeOutOfOrder {
            field: CrontabField::Day,
            start: 31,
            end: 1,
        })
    );
}

#[test]
fn crontab_fields_display_names_and_bounds() {
    let cases = [
        (CrontabField::Minute, "minute", (0, 59)),
        (CrontabField::Hour, "hour", (0, 23)),
        (CrontabField::Day, "day", (1, 31)),
        (CrontabField::Month, "month", (1, 12)),
    ];

    for (field, name, bounds) in cases {
        assert_eq!(field.to_string(), name);
        assert_eq!(field.bounds(), bounds);
    }
}

#[test]
fn crontab_value_match_value() {
    assert!(CrontabValue::Number(30).match_value(30, 0));
    assert!(CrontabValue::Range(8, 10).match_value(8, 0));
    assert!(!CrontabValue::Range(8, 10).match_value(7, 0));
    assert!(CrontabValue::Step(4).match_value(5, 1));
    assert!(CrontabValue::Step(9).match_value(9, 0));
    assert!(!CrontabValue::Step(9).match_value(9, 1));
    assert!(!CrontabValue::Step(0).match_value(9, 0));
    assert!(CrontabValue::Any.match_value(16, 0));
}

#[test]
fn crontab_should_run_at() -> Result<()> {
    let crontab = Crontab {
        timer: CrontabTimer {
            minutes: vec![CrontabValue::Number(30)],
            hours: vec![CrontabValue::Range(8, 10)],
            days: vec![CrontabValue::Step(4)],
            ..Default::default()
        },
        task_identifier: "test".to_string(),
        ..Default::default()
    };
    assert!(crontab.should_run_at(&"2012-12-17T08:30:12".parse()?));
    assert!(crontab.should_run_at(&"2015-02-05T09:30:00".parse()?));
    assert!(crontab.should_run_at(&"1998-10-13T10:30:59".parse()?));

    assert!(!crontab.should_run_at(&"2012-12-17T11:30:59".parse()?));
    assert!(!crontab.should_run_at(&"2015-02-05T09:31:00".parse()?));
    assert!(!crontab.should_run_at(&"2012-12-13T08:29:12".parse()?));
    assert!(!crontab.should_run_at(&"1998-10-04T10:30:59".parse()?));

    Ok(())
}
