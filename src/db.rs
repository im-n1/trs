use std::path::PathBuf;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::NaiveDate;
use gtfs_structures::{Gtfs, Stop, Trip};
use rayon::prelude::*;
use reqwest;
use serde::{Deserialize, Serialize};
use spinners::{Spinner, Spinners};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomCalendar {
    pub monday: bool,
    pub tuesday: bool,
    pub wednesday: bool,
    pub thursday: bool,
    pub friday: bool,
    pub saturday: bool,
    pub sunday: bool,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

impl From<&gtfs_structures::Calendar> for CustomCalendar {
    fn from(cal: &gtfs_structures::Calendar) -> Self {
        Self {
            monday: cal.monday.clone(),
            tuesday: cal.tuesday.clone(),
            wednesday: cal.wednesday.clone(),
            thursday: cal.thursday.clone(),
            friday: cal.friday.clone(),
            saturday: cal.saturday.clone(),
            sunday: cal.sunday.clone(),
            start_date: cal.start_date.clone(),
            end_date: cal.end_date.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub route: String, // human readable line name
    pub trip: String,
    pub calendar: CustomCalendar,
    pub stop_time: Option<u32>,
    pub stop: String,
}

#[derive(Serialize, Deserialize)]
pub struct Database {
    // TODO: vec -> array
    pub records: Vec<Record>,
}

impl<'a> Database {
    pub fn from(gtfs: &'a Gtfs, stop: Arc<Stop>) -> Result<Self, Box<dyn std::error::Error>> {
        let records = Self::fetch(gtfs, stop)?;
        // Self::debug(routes_and_calendars);

        Ok(Self { records })
    }

    /// Walks thru all stops and collects all trips that intersect any
    /// of selected stop.
    /// Uses parallel iterating (rayon)
    fn fetch(gtfs: &'a Gtfs, stop: Arc<Stop>) -> Result<Vec<Record>, Box<dyn std::error::Error>> {
        let records = Arc::new(Mutex::new(vec![]));

        let sp = Spinner::new(Spinners::Line, "Fetching times (can take minutes)".into());

        gtfs.routes.par_iter().for_each(|(_, route)| {
            let records = Arc::clone(&records);

            for (_, trip) in gtfs
                .trips
                .par_iter()
                .filter(|trip| trip.1.route_id == route.id)
                .collect::<HashMap<&String, &Trip>>()
            {
                for time in trip.stop_times.iter() {
                    if time.stop.id == stop.id {
                        records.lock().unwrap().push(Record {
                            route: route.short_name.clone(),
                            trip: trip.service_id.clone(),
                            calendar: CustomCalendar::from(
                                gtfs.get_calendar(trip.service_id.as_str()).unwrap(),
                            ),
                            stop_time: time.arrival_time.clone(),
                            stop: time.stop.name.clone(),
                        });
                    }
                }
            }
        });

        sp.stop();

        Ok(Mutex::into_inner(Arc::try_unwrap(records).unwrap()).unwrap())
    }
}

/// Represents GTFS file wrapper for manipulation like downloading or parsing.
pub struct DataFile {
    remote_location: String,
    local_location: PathBuf,
}

impl DataFile {
    /// Constructor.
    pub fn new(conf_dir: &PathBuf, remote_location: String) -> Self {
        let mut local_location = conf_dir.clone();
        local_location.push("data_file.gtfs");

        Self {
            remote_location,
            local_location,
        }
    }

    /// Downloads or copies the data file into config folder.
    /// TODO: custom error
    pub async fn retrieve(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        // Download from the internet
        // or copy from existing location.
        if self.remote_location.starts_with("http") {
            fs::write(
                &self.local_location,
                reqwest::get(&self.remote_location).await?.bytes().await?,
            )
            .await?;
        } else {
            fs::copy(&self.remote_location, &self.local_location).await?;
        }

        Ok(self.local_location.clone())
    }

    /// Parses earlier downloaded GTFS file.
    pub fn parse(&self) -> Result<Gtfs, Box<dyn std::error::Error>> {
        Ok(Gtfs::new(self.local_location.to_str().unwrap())?)
    }
}