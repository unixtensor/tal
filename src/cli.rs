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
	user: bool,
	/// List system installed applications that are located in /usr/share/applications
	#[arg(long, short)]
	system: bool,
	/// List flatpak applications that are located in /var/lib/flatpak/exports/share/applications/
	#[arg(long, short)]
	flatpak: bool,
	/// List both system and user applications
	#[arg(long, short)]
	all: bool,
	/// Show details about the application entries
	#[arg(long, short)]
	details: bool,
	/// Send application output to stdout
	#[arg(long, short)]
	output: bool,
}

pub fn parser() -> Option<()> {
	let cli_parser = Cli::parse();

	if let Some(app_names) = cli_parser.input {
		app_names.into_iter().for_each(|app_name| {
			if let Err(e) = apps::Spawn::new(app_name, env::var("TERMINAL").ok()).run(cli_parser.output) {
				eprintln!("{e}")
			};
		});
		return None
	}
	if cli_parser.all {
		match apps::Installed.all() {
			Ok(entries) => apps::Display::new(cli_parser.details).names(entries),
			Err(e) => eprintln!("{e}"),
		}
		return None
	}
	if cli_parser.user {
		apps::Display::new(cli_parser.details).entries(apps::Installed.user());
	}
	if cli_parser.system {
		apps::Display::new(cli_parser.details).entries(apps::Installed.system());
	}
	if cli_parser.flatpak {
		apps::Display::new(cli_parser.details).entries(apps::Installed.flatpak());
	}
	None
}