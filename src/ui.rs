use std::fmt::{self, Display};
use std::io::{self, prelude::*, BufRead};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Local, NaiveTime};
use clap::ArgMatches;
use gtfs_structures::{Gtfs, Stop};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use spinners::{Spinner, Spinners};

use crate::db::DataFile;
use crate::timetables::Departure;

pub struct WizardOutput {
    pub gtfs: Gtfs,
    pub stops: Vec<FoundStop>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FoundStop {
    pub id: String,
    pub stop: Arc<Stop>,
    pub terminating_stop: Arc<Stop>,
}

/// Implementing Display trait so the stop can be printed out.
impl Display for FoundStop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.stop, self.terminating_stop)
    }
}

/// Wizard for user that ask a few questions.
/// The result is used by Config struct.
pub struct Wizard<'a> {
    pub data_file_url: Option<String>,
    conf_dir: &'a PathBuf,
    pub data_file_path: Option<PathBuf>,
}

impl<'a> Wizard<'a> {
    pub async fn new(conf_dir: &'a PathBuf) -> Wizard<'a> {
        Wizard {
            data_file_url: None,
            conf_dir,
            data_file_path: None,
        }
    }

    /// Kick offs wizard which returns output containing GTFS data and user's chosen stops.
    pub async fn run_wizard(&mut self) -> Result<WizardOutput, Box<dyn std::error::Error>> {
        let gtfs = self.determine_retrieve_and_parse_data_file().await?;
        let stops = self.read_stop_names(&gtfs)?;

        Ok(WizardOutput { gtfs, stops })
    }

    /// Downloads or copies (depends on the origin location) the datafile
    /// to project config location (see Config.path) and parses it's content
    /// which is then returned.
    async fn determine_retrieve_and_parse_data_file(
        &mut self,
    ) -> Result<Gtfs, Box<dyn std::error::Error>> {
        // Determine (read) data file path/URL.
        println!("Enter data file path/URL: ");
        let mut data_file = String::new();
        io::stdin().lock().read_line(&mut data_file)?;
        data_file = data_file.trim().to_owned();
        self.data_file_url = Some(data_file.clone());

        // Play with datafile.
        let df = DataFile::new(self.conf_dir, data_file);

        // Spinner - start.
        let mut sp = Spinner::new(Spinners::Line, "fetching times...".into());
        // io::stdout().flush()?;

        // Retrieve data file.
        self.data_file_path = Some(df.retrieve().await?);

        // Spinner - stop.
        sp.stop();
        println!("done");

        // Spinner - start.
        let mut sp = Spinner::new(Spinners::Line, "parsing...".into());
        io::stdout().flush().unwrap();

        // Parse.
        let gtfs = df.parse()?;

        // Spinner - stop.
        sp.stop();
        println!("done!");

        Ok(gtfs)
    }

    /// Triggers the loop for reading stop names. User can
    /// enter as many stops as he likes.
    pub fn read_stop_names(
        &self,
        gtfs: &'a Gtfs,
    ) -> Result<Vec<FoundStop>, Box<dyn std::error::Error>> {
        let mut chosen_stops = vec![];

        loop {
            // Read stop name.
            chosen_stops.push(self.read_stop_name(gtfs)?);

            // Ask for more stops.
            if !Ui::confirm(
                format!(
                    "Currently {} stop(s) has been chosen. Do you want to add another one?",
                    chosen_stops.len()
                )
                .as_str(),
            ) {
                break;
            }
        }

        Ok(chosen_stops)
    }

    /// Tries to collect one stop based on user input.
    fn read_stop_name(&self, gtfs: &'a Gtfs) -> Result<FoundStop, Box<dyn std::error::Error>> {
        loop {
            let mut found_stops = self.seek_stops(gtfs)?;

            println!("Found {} stops:", found_stops.len());

            // Sort found stops by stop name.
            found_stops.sort_by_key(|i| i.1.name.clone());

            // Paralelly iterate thru stops and fetch terminating station for
            // each station (based on trip direction).
            let found_stops_with_terminating_stop: Vec<FoundStop> = found_stops
                .par_iter()
                .map(|item| FoundStop {
                    id: item.0.clone(),
                    stop: item.1.clone(),
                    terminating_stop: self.get_terminating_trip_stop_for_stop(gtfs, item.1.clone()),
                })
                .collect();

            // TODO: Use Ui::select_stop()
            let stop = Ui::select_stop(
                "Please enter the number of stop you want to choose:",
                &found_stops_with_terminating_stop,
            );

            if stop.is_ok() {
                return Ok(stop.unwrap().clone());
            }
            // Print stops.
            // for (i, chosen_stop) in found_stops_with_terminating_stop.iter().enumerate() {
            //     Ui::print_stop_record(i, &chosen_stop)
            // }

            // // Read stop number.
            // let stop_number: usize;

            // loop {
            //     println!("Please enter the number of stop you want to choose:");
            //     let mut stop_number_input = String::new();
            //     io::stdin().lock().read_line(&mut stop_number_input)?;

            //     // Validate stop number.
            //     match stop_number_input.trim().parse::<usize>() {
            //         Ok(number) => {
            //             stop_number = number;
            //             break;
            //         }
            //         Err(_) => {
            //             println!("Wrong number! Try again.");
            //         }
            //     }
            // }

            // if let Some(stop) = found_stops_with_terminating_stop.get(stop_number) {
            //     return Ok(stop.clone());
            // }

            // println!("Wrong number! Try again.");
        }
    }

    /// Asks user for input and then finds similar stops in datafile.
    /// All similar stops are then returned.
    /// If no similar stop are found user is asked for the input again.
    fn seek_stops(
        &self,
        gtfs: &'a Gtfs,
    ) -> Result<Vec<(String, Arc<Stop>)>, Box<dyn std::error::Error>> {
        let mut found_stops: Vec<(String, Arc<Stop>)>;

        loop {
            println!("Enter stop name: ");
            let mut stop = String::new();
            io::stdin().lock().read_line(&mut stop)?;
            stop = stop.trim().to_owned();

            // Validate stop name against data file.
            found_stops = gtfs
                .stops
                .iter()
                .filter(|x| x.1.name.contains(stop.as_str()))
                .map(|x| (x.0.clone(), x.1.clone()))
                .collect();

            // We did found at least one stop.
            if !found_stops.is_empty() {
                break;
            }

            println!("No stop with such name (or similar) was found. Please try again.");
        }

        Ok(found_stops)
    }

    /// Seeks last stop (terminating station) for the given stop (based on
    /// associated trip and stop times.
    fn get_terminating_trip_stop_for_stop(&self, gtfs: &'a Gtfs, stop: Arc<Stop>) -> Arc<Stop> {
        let mut found_stop: Option<Arc<Stop>> = None;

        // Closes thing to stops we have are trips.
        for (_, trip) in gtfs.trips.iter() {
            for time in trip.stop_times.iter() {
                if time.stop.id == stop.id {
                    found_stop = Some(
                        trip.stop_times
                            .last()
                            .expect("Trip ha no stop - that's very weird!")
                            .stop
                            .clone(),
                    );
                }
            }
        }

        if found_stop.is_none() {
            return stop;
        }

        found_stop.unwrap()
    }
}

pub struct UiConfig {
    limit: usize,
}

pub struct Ui {
    config: UiConfig,
}

impl Ui {
    pub fn new(args: ArgMatches) -> Self {
        Self {
            config: Ui::process_args(args),
        }
    }

    pub fn process_args(args: ArgMatches) -> UiConfig {
        // -l argument
        let limit = args
            .get_one::<String>("limit")
            .unwrap()
            .parse::<usize>()
            .unwrap();

        UiConfig { limit }
    }

    pub fn output(&self, departures: Vec<Departure>) {
        self.print_default(departures)
    }

    /// Prints departures in default format:
    ///
    /// Novovysočanská -> Sídliště Čakovice
    /// -----------------------------------
    /// 903 - 23:15
    /// 913 - 23:58
    /// 903 - 00:15
    ///
    fn print_default(&self, mut departures: Vec<Departure>) {
        // Sort  by stop name.
        departures.sort_by(|a, b| a.stop.name.partial_cmp(&b.stop.name).unwrap());

        for departure in departures.iter() {
            // Heading.
            let heading = format!(
                "{} -> {}",
                departure.stop.name, &departure.stop.terminating_stop
            );
            println!("");
            println!("{}", heading);
            println!("{}", "-".repeat(heading.chars().count()));

            let now = Local::now();

            // Timetable.
            for departure_record in departure.departures.iter().take(self.config.limit.clone()) {
                if departure_record.stop_time.is_some() {
                    let departure = NaiveTime::from_num_seconds_from_midnight(
                        departure_record.stop_time.unwrap(),
                        0,
                    );

                    println!(
                        "{} - {} (+{} min)",
                        departure_record.route,
                        departure.format("%H:%M"),
                        (departure - now.time()).num_minutes()
                    );
                }
            }
        }
    }

    /// Renders prompt with the given stops and prompt messages and let's
    /// user choose one stop which is then returned
    pub fn select_stop<'a>(
        prompt: &str,
        stops: &'a Vec<FoundStop>,
    ) -> Result<&'a FoundStop, Box<dyn std::error::Error>> {
        // 1. print stop choices.
        for (i, stop) in stops.iter().enumerate() {
            Self::print_stop_record(i, stop);
        }

        // 2. let user enter the number of a stop he wants to delete.
        loop {
            println!("{}", prompt);
            let mut the_stop_number_input = String::new();
            io::stdin().lock().read_line(&mut the_stop_number_input)?;

            if let Ok(the_stop_index) = the_stop_number_input.trim().parse::<usize>() {
                let the_stop = stops.iter().nth(the_stop_index);

                if the_stop.is_some() {
                    return Ok(the_stop.unwrap());
                }
            }

            println!("Wrong number! Try again.");
        }
    }

    /// Prints stop record as:
    /// 1) Stop -> TerminatingStop
    pub fn print_stop_record(number: usize, stop: &FoundStop) {
        println!("{}) {}", number, stop);
    }

    /// Prints out message in level "info".
    pub fn info(msg: &str) {
        println!("{}", msg);
    }

    /// Prints confirm dialog where "y" answer confirms and anything else denies
    /// the request.
    pub fn confirm(msg: &str) -> bool {
        loop {
            print!("{}", msg);
            println!(" (y/n)");

            let mut answer = String::new();
            io::stdin().lock().read_line(&mut answer).unwrap();
            answer = answer.trim().to_owned();

            if "y" == answer.to_lowercase() {
                return true;
            }

            return false;
        }
    }
}
