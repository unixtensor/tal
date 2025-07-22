use clap::{Parser};

use crate::apps::{self};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
	/// Launch an application from command line
	input: Option<Vec<String>>,
	/// List user installed apps that are located in /home/USER/.local/share/applications
	#[arg(long, short)]
	user_apps: bool,
	/// List system installed apps that are located in /usr/share/applications
	#[arg(long, short)]
	system_apps: bool,
	/// List both system and user apps
	#[arg(long, short)]
	all_apps: bool,
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
			if apps::Installed.run_cli(&app_name, cli_parser.output).is_none() {
				eprintln!("Application {app_name:?} does not exist.")
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