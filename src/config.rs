use std::io::{self, prelude::*};
use std::path::PathBuf;

use async_trait::async_trait;
use clap::ArgMatches;
use gtfs_structures::Gtfs;
use serde::{Deserialize, Serialize};
use serde_yaml;
use spinners::{Spinner, Spinners};
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;

use crate::app::{ArgSignal, ArgumentProcessResult};
use crate::db::{DataFile, Database};
use crate::ui::{FoundStop, Ui, Wizard};

const CONF_DIR: &str = "transpors";
const CONF_FILE: &str = "config.yaml";

#[derive(Serialize, Deserialize)]
pub struct Stop {
    id: String,
    pub name: String,
    pub terminating_stop: String,
    pub database: Database,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    data_file_url: String,
    data_file_path: PathBuf,
    pub user_stops: Vec<FoundStop>,
    pub stops: Vec<Stop>,
}

impl Config {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config: Self;

        // Determine config path.
        let dir = Self::determine_conf_dir();

        // Check if config exists.
        if !dir.exists() {
            Self::create_conf_dir(&dir).await?;
            let mut wiz = Wizard::new(&dir).await;
            let output = wiz.run_wizard().await.expect("Wizard failed. Exiting.");
            let stops = Self::build_stops_database(&output.gtfs, &output.stops).await;

            config = Self {
                data_file_url: wiz.data_file_url.unwrap(),
                data_file_path: wiz.data_file_path.unwrap().clone(),
                user_stops: output.stops,
                stops,
            };

            config.save().await?;
        } else {
            config = Self::load().await?;
        }

        // Debug output
        // for stop in &config.stops {
        //     println!(
        //         "Stop {} has {} records in database",
        //         &stop.name,
        //         stop.database.records.len()
        //     );
        // }

        Ok(config)
    }

    /// Loads config file and constructs self.
    async fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Load config file.
        let mut config_file = File::open(Self::determine_conf_file_path()).await?;
        let mut file_content = String::new();
        config_file.read_to_string(&mut file_content).await?;

        // Construct Self.
        Ok(serde_yaml::from_str(&file_content)?)
    }

    /// Determines main config ditectory (wrapper for all app files).
    fn determine_conf_dir() -> PathBuf {
        let mut dir = dirs::config_dir().expect("Config directory is not available.");
        dir.push(CONF_DIR);

        dir
    }

    /// Determines main config file path.
    fn determine_conf_file_path() -> PathBuf {
        let mut path = Self::determine_conf_dir();
        path.push(CONF_FILE);

        path
    }

    /// Creates config directory and returns path to that directory.
    async fn create_conf_dir(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if !path.exists() {
            fs::create_dir_all(&path).await?;
        }

        Ok(())
    }

    /// Saves config (serialize) to config YAML file.
    async fn save(&self) -> Result<(), std::io::Error> {
        fs::write(
            Self::determine_conf_file_path(),
            serde_yaml::to_string(self).expect("Couldn't serialize config."),
        )
        .await
    }

    /// Builds up stop database for each stop from config.
    async fn build_stops_database(gtfs: &Gtfs, stops: &Vec<FoundStop>) -> Vec<Stop> {
        let mut processed_stops = vec![];
        let mut sp = Spinner::new(Spinners::Line, "fetching times...".into());

        // TODO: implement rayon
        for found_stop in stops {
            // TODO: remove unwrap set up error.
            let database = Database::from(&gtfs, found_stop.stop.clone()).unwrap();
            processed_stops.push(Stop {
                id: found_stop.id.clone(),
                name: found_stop.stop.name.clone(),
                terminating_stop: found_stop.terminating_stop.name.clone(),
                database,
            });
        }

        sp.stop();
        println!("done");

        processed_stops
    }

    /// Downloads or copies (depends on the origin location) the datafile
    /// to project config location (see Config.path) and parses it's content.
    /// Stops database is then rebuilded and saved.
    async fn refresh_data_file(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 1. download
        let df = DataFile::new(&Self::determine_conf_dir(), self.data_file_url.clone());

        // Spinner - start.
        let mut sp = Spinner::new(Spinners::Line, "retrieving...".into());
        io::stdout().flush()?;

        df.retrieve().await?;

        // Spinner - stop.
        sp.stop();
        println!("done");

        // 2. parse
        // Spinner - start.
        let mut sp = Spinner::new(Spinners::Line, "parsing...".into());
        // io::stdout().flush().unwrap();

        let gtfs = df.parse()?;

        // Spinner - stop.
        sp.stop();
        println!("done");

        // 3. build database.
        self.stops = Config::build_stops_database(&gtfs, &self.user_stops).await;

        // 4. Save config.
        self.save().await?;

        Ok(())
    }

    /// Removed whole config directory.
    async fn wipe(&self) -> Result<(), std::io::Error> {
        fs::remove_dir_all(Self::determine_conf_dir()).await
    }

    /// Loads existing (already downloaded) GTFS data file
    /// while showing loading spinners.
    fn get_gtfs_file(&self) -> Result<Gtfs, Box<dyn std::error::Error>> {
        let conf_dir = Self::determine_conf_dir();
        let df = DataFile::new(&conf_dir, self.data_file_url.clone());
        let mut sp = Spinner::new(
            Spinners::Line,
            "parsing data file (can take minutes)...".into(),
        );
        io::stdout().flush().unwrap();
        let gtfs = df.parse()?;
        sp.stop();
        println!("done");

        Ok(gtfs)
    }
}

#[async_trait]
impl ArgSignal for Config {
    /// Handles following arguments:
    /// -r
    /// -a
    /// -d
    /// -w
    async fn processs_args(
        &mut self,
        args: ArgMatches,
    ) -> Result<ArgumentProcessResult, Box<dyn std::error::Error>> {
        // -r argument
        if args.is_present("refresh") {
            self.refresh_data_file().await?;
        }

        // -a argument
        if args.is_present("add-stop") {
            // 1. parse GTFS file.
            let conf_dir = Self::determine_conf_dir();
            let gtfs = self.get_gtfs_file()?;

            // 2. read new stops.
            let wiz = Wizard::new(&conf_dir).await;
            // TODO: Check duplicity
            self.user_stops.append(&mut wiz.read_stop_names(&gtfs)?);

            // 3. build stop database.
            self.stops = Self::build_stops_database(&gtfs, &self.user_stops).await;

            // 4. save config
            self.save().await?;
        }

        // -d argument
        if args.is_present("delete-stop") {
            // 1. determine the stop.
            let to_be_removed = Ui::select_stop(
                "Please enter the number of stop you want to delete:",
                &self.user_stops,
            )?;

            // 2. parse GTFS file.
            let gtfs = self.get_gtfs_file()?;

            // 3. remove the stop.
            for (i, stop) in self.user_stops.iter().enumerate() {
                if stop.id == to_be_removed.id {
                    let stop = self.user_stops.remove(i);
                    let msg = format!("Stop {} has been removed.", &stop);
                    Ui::info(&msg);

                    // 4. build stop database.
                    self.stops = Self::build_stops_database(&gtfs, &self.user_stops).await;
                    self.save().await?;

                    break;
                }
            }
        }

        // -w argument
        if args.is_present("wipe") {
            if Ui::confirm("Do you want to wipe whole app config?") {
                self.wipe().await?;
                return Ok(ArgumentProcessResult::Stop);
            }
        }

        Ok(ArgumentProcessResult::Continue)
    }
}
