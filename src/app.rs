// use std::io::prelude::*;
use std::rc::Rc;

use async_trait::async_trait;
use clap::ArgMatches;

use crate::config::Config;
use crate::timetables::Timetables;
use crate::ui::Ui;

pub struct App {}

/// States of CLI argument processing result. Sometimes we want
/// the app to continue to run but sometimes argument processing
/// done all the work and we want to stop the process.
/// See Config.processs_args().
pub enum ArgumentProcessResult {
    Stop,
    Continue,
}

#[async_trait]
pub trait ArgSignal {
    async fn processs_args(
        &mut self,
        args: ArgMatches,
    ) -> Result<ArgumentProcessResult, Box<dyn std::error::Error>>;
}

impl App {
    pub async fn run(args: ArgMatches) -> Result<(), Box<dyn std::error::Error>> {
        // Create/get config (also handles first start).
        let mut config = Rc::new(Config::new().await?);

        // (we have config now).
        // Process all arguments.
        let result = Rc::get_mut(&mut config)
            .unwrap()
            .processs_args(args.clone())
            .await?;

        if let ArgumentProcessResult::Continue = result {
            // Always print timetables.
            // Fetch valid/relevant timetables.
            let timetables = Timetables::new(config.clone()).await;
            let departures = timetables.get_departures();

            // Render timetables.
            Ui::new(args.clone()).output(departures).await;
        }

        Ok(())
    }
}
