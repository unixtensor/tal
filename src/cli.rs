use clap::{Parser};
use std::env;

use crate::apps::{self};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
	/// Launch applications from the command line
	input: Option<Vec<String>>,
	/// List user installed applications that are located in /home/USER/.local/share/applications
	#[arg(long, short)]
	user_apps: bool,
	/// List system installed applications that are located in /usr/share/applications
	#[arg(long, short)]
	system_apps: bool,
	/// List both system and user applications
	#[arg(long, short)]
	all_apps: bool,
	/// Show details about the application entries
	#[arg(long, short)]
	details: bool,
	/// Send application output to stdout
	#[arg(long, short)]
	output: bool,
}

fn terminal_env() -> Option<String> {
	env::var("TERMINAL").ok()
}

pub fn parser() -> Option<()> {
	let cli_parser = Cli::parse();
	if let Some(app_names) = cli_parser.input {
		app_names.into_iter().for_each(|app_name| {
			if let Err(run_err) = apps::Installed.run(app_name, cli_parser.output) {
				eprintln!("{run_err}")
			};
		});
		return None
	}
	if cli_parser.all_apps {
		match apps::Installed.all() {
			Ok(entries) => apps::Display::new(cli_parser.details).names(entries),
			Err(e) => eprintln!("{e}"),
		}
		return None
	}
	if cli_parser.user_apps {
		apps::Display::new(cli_parser.details).entries(apps::Installed.user());
	}
	if cli_parser.system_apps {
		apps::Display::new(cli_parser.details).entries(apps::Installed.system());
	}
	None
}