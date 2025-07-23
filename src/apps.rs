use std::{collections::HashMap, env, fmt, fs::{self, ReadDir}, path::PathBuf, process::{Command, Stdio}};
use thiserror::Error;

const TERM_EMULATORS: &[&str] = &[
	""
];

#[derive(Debug)]
pub struct IniAction {
	pub name: Option<String>,
	pub exec: Option<String>,
	pub terminal: Option<bool>,
}

#[derive(Debug)]
pub struct Ini {
	pub name: String,
	pub exec: String,
	pub terminal: bool,
	pub actions: HashMap<String, IniAction>,
}

#[derive(Debug)]
struct IniHeader<'a> {
	name: Option<&'a str>,
	exec: Option<&'a str>,
	terminal: Option<bool>,
}

#[derive(Debug, Error)]
pub enum Error {
	#[error("Failed to get system applications.")]
	System,
	#[error("Failed to get user applications.")]
	User,
}

impl fmt::Display for Ini {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Name={}\n\t- Exec={}\n\t- Terminal={}", self.name, self.exec, self.terminal)
	}
}

fn into_list(read_dir: Option<ReadDir>) -> Option<Vec<PathBuf>> {
	read_dir.map(|apps_dir| {
		apps_dir.filter_map(Result::ok).filter_map(|entry| {
			let path = entry.path();
			let f_type = entry.file_type().ok()?;
			match (f_type.is_file() || f_type.is_symlink()) && path.extension()? == "desktop" {
				true => Some(path),
				false => None,
			}
		}).collect()
	})
}

fn app_entry(apps: &[PathBuf]) -> Vec<Ini> {
	apps.iter().filter_map(|app| {
		fs::read(app).ok().and_then(|bytes| String::from_utf8(bytes).ok()).and_then(decode_ini)
	}).collect()
}

fn decode_ini(app_entry_content: String) -> Option<Ini> {
	let ini_lines: Vec<&str> = app_entry_content.split('\n').collect();
	match ini_lines.first() {
		Some(&"[Desktop Entry]") => (),
		_ => return None,
	}

	let mut ini_header = IniHeader {
		name: None,
		exec: None,
		terminal: None
	};
	let mut ini_action_current: Option<&str> = None;
	let mut ini_actions: HashMap<String, IniAction> = HashMap::new();

	for line in ini_lines {
		if line.starts_with("#") { continue; } //ini comment
		if let Some(action_section_name) = ini_action_current { //are we in an action section
			let (key, val) = match line.split_once('=') {
				Some(kv) => kv,
				None => continue,
			};
			ini_actions.entry(action_section_name.to_owned()).and_modify(|action| {
				if key == "Name" {
					action.name = Some(val.to_owned())
				} else if key == "Exec" {
					action.exec = Some(val.to_owned())
				} else if key == "Terminal" {
					action.terminal = match val {
						"False" | "false" => Some(false),
						"True" | "true" => Some(true),
						_ => Some(false),
					}
				}
			});
			if let Some(action) = ini_actions.get(action_section_name) {
				if action.name.is_some() && action.exec.is_some() && action.terminal.is_some() {
					ini_action_current = None;
				}
			};
			if line.starts_with("[Desktop Action ") && line.ends_with("]") {
				if let Some((_, right)) = line.split_once("Action ") {
					let action_section = &right[..right.len() - 1]; //trim "]"
					ini_action_current = Some(action_section);
					ini_actions.insert(action_section.to_owned(), IniAction {
						name: None,
						exec: None,
						terminal: None
					});
				}
			};
			continue;
		}
		if line.starts_with("[Desktop Action ") && line.ends_with("]") {
			if let Some((_, right)) = line.split_once("Action ") {
				let action_section = &right[..right.len() - 1]; //trim "]"
				ini_action_current = Some(action_section);
				ini_actions.insert(action_section.to_owned(), IniAction {
					name: None,
					exec: None,
					terminal: None
				});
			}
			continue;
		};

		let (key, val) = match line.split_once('=') {
			Some(kv) => kv,
			None => continue,
		};
		match key {
			"Name" => ini_header.name = Some(val),
			"Exec" => ini_header.exec = Some(val),
			"Terminal" => ini_header.terminal = match val {
				"False" | "false" => Some(false),
				"True" | "true" => Some(true),
				_ => Some(false),
			},
			_ => continue,
		}
	}
	if ini_header.terminal.is_none() {
		ini_header.terminal = Some(false);
	}
	if let (Some(name), Some(exec), Some(terminal)) = (ini_header.name, ini_header.exec, ini_header.terminal) {
		return Some(Ini {
			name: name.to_owned(),
			exec: exec.to_owned(),
			terminal,
			actions: ini_actions
		});
	}
	None
}

fn exec(app: Ini, stdout: bool) -> Option<()> {
	let mut args: Vec<String> = app.exec.split_whitespace()
    	.filter(|s| !matches!(*s, "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%k" | "%v" | "%m" | "%c" | "%i" | "%s"))
		.map(|s| s.to_owned())
		.collect();
	if app.terminal {
		args.insert(0, "ghostty".to_owned());
		args.insert(1, "-e".to_owned());
	}
	match Command::new(args.remove(0))
		.args(args) //The dedup police cant arrest me...
		.stdout(if stdout { Stdio::inherit() } else { Stdio::null() })
		.stderr(if stdout { Stdio::inherit() } else { Stdio::null() })
		.spawn()
	{
		Ok(mut child_proc) => {
			println!("Launching application {:?}.", app.name);
			if stdout {
				child_proc.wait().ok();
			}
			return Some(())
		},
		Err(spawn_err) => eprintln!("{spawn_err}")
	}
	None
}

pub struct Installed;
impl Installed {
	pub const UNIX_USER_APPS_PATH: &str = ".local/share/applications";
	pub const UNIX_SYS_APPS_PATH: &str = "/usr/share/applications";

	pub fn run_cli(&self, app_name: &str, stdout: bool) -> Option<()> {
		match Installed.all() {
		    Ok(all_apps) => for app_entry in all_apps.into_iter() {
				if app_entry.name.to_lowercase() == app_name.to_lowercase() {
					return exec(app_entry, stdout);
				};
			},
		    Err(err) => eprintln!("{err}"),
		}
		None
	}

	pub fn system(&self) -> Option<Vec<Ini>> {
		let sys_apps = into_list(fs::read_dir(Self::UNIX_SYS_APPS_PATH).ok())?;
		Some(app_entry(&sys_apps))
	}

	pub fn user(&self) -> Option<Vec<Ini>> {
		let user_apps = into_list(env::home_dir().map(|mut home| {
			home.push(Self::UNIX_USER_APPS_PATH);
			home
		}).and_then(|user_apps| fs::read_dir(user_apps).ok()))?;
		Some(app_entry(&user_apps))
	}

	pub fn all(&self) -> Result<Vec<Ini>, Error> {
		let mut user_apps = self.user().ok_or(Error::User)?;
		let mut sys_apps = self.system().ok_or(Error::System)?;
		user_apps.append(&mut sys_apps);
		Ok(user_apps)
	}
}

pub struct Display(bool);
impl Display {
	pub const fn new(show_details: bool) -> Self {
		Self(show_details)
	}

	#[inline]
	pub fn actions(&self, actions: HashMap<String, IniAction>) {
		if actions.is_empty() { return; }
		print!("\n\t");
		actions.into_iter().for_each(|(act_name, act)| {
			println!("{act_name}\n\t- Name={}\n\t- Exec={}\n\t- Terminal={}",
				act.name.unwrap_or("None".to_owned()),
				act.exec.unwrap_or("None".to_owned()),
				act.terminal.unwrap_or(false))
		});
	}

	#[inline]
	pub fn names(&self, entries: Vec<Ini>) {
		entries.into_iter().for_each(|app| match self.0 {
			true => {
				println!("{app}");
				self.actions(app.actions);
			},
			false => println!("{}", app.name)
		});
	}

	#[inline]
	pub fn entries(&self, entries_maybe: Option<Vec<Ini>>) {
		match entries_maybe {
			Some(entries) => self.names(entries),
			None => eprintln!("Failed to display entries."),
		}
	}
}
