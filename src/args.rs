use clap::{App, AppSettings, Arg, ArgMatches};

/// Default limit for departures to be printed out.
const DEPARTURES_COUNT: &str = "3";

pub fn parse() -> ArgMatches {
    App::new("TranspoRS")
        .setting(AppSettings::ColoredHelp)
        .version("0.1.2")
        .author("Hrdina Pavel <hrdina.pavel@gmail.com>")
        .about("Transportation timetables for command line.")
        .arg(
            Arg::with_name("refresh")
                .short('r')
                .help("Fetches fresh data file from source and rebuilds timetable database."),
        )
        .arg(
            Arg::with_name("add-stop")
                .short('a')
                .help("Adds one stop to user's stops configuration."),
        )
        .arg(
            Arg::with_name("delete-stop")
                .short('d')
                .help("Delete one stop from user's stops configuration."),
        )
        .arg(
            Arg::with_name("wipe")
                .short('w')
                .help("Wipes whole config. Cannot be undone, be careful."),
        )
        .arg(
            Arg::with_name("limit")
                .short('l')
                .takes_value(true)
                .default_value(DEPARTURES_COUNT)
                .help("Limits number of departures from each stop."),
        )
        .get_matches()
}
