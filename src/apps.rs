use std::{collections::HashMap, env, fmt, fs::{self, ReadDir}, io::Error, path::PathBuf, process::{Command, Stdio}};
use thiserror::Error;

type Actions = HashMap<String, IniAction<String>>;

#[derive(Debug)]
pub struct IniAction<T: AsRef<str>> {
	pub name: Option<T>,
	pub exec: Option<T>,
	pub terminal: Option<bool>,
}

#[derive(Debug)]
pub struct Ini {
	pub name: String,
	pub exec: String,
	pub terminal: bool,
	pub actions: Actions,
}

#[derive(Debug, Error)]
pub enum RunError {
	#[error("Failed to get system applications.")]
	System,
	#[error("Failed to get user applications.")]
	User,
	#[error("Failed to get flatpak applications.")]
	Flatpak,
	#[error("Application {0:?} failed to start because it doesn't know what terminal to use...")]
	NoTerminal(String),
	#[error("An error occured executing the application, most likely a terminal does not exist.\n{0}")]
	Exec(Error),
	#[error("Application {0:?} does not exist.")]
	NotFound(String),
}

impl fmt::Display for Ini {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "Name={}\n\t- Exec={}\n\t- Terminal={}", self.name, self.exec, self.terminal)
	}
}

struct ApplicationEntry(String);
impl ApplicationEntry {
	pub const fn new(entry_inner: String) -> Self {
		Self(entry_inner)
	}
	const fn body<T: AsRef<str>>(&self) -> IniAction<T> {
		IniAction { name: None, exec: None, terminal: None }
	}

	#[inline]
	fn lines(&self) -> Option<Vec<&str>> {
		let ini_lines: Vec<&str> = self.0.split("\n")
			.filter(|line| !(*line).starts_with("#")) //Filter out comments
			.collect();
		match ini_lines.first() {
			Some(&"[Desktop Entry]") => Some(ini_lines),
			_ => None,
		}
	}

	#[inline]
	fn is_action<'a>(&self, line: &'a str) -> Option<&'a str> {
		if let Some((_, right)) = line.split_once("Action ") {
			let action_section = &right[..right.len() - 1]; //trim "]"
			return Some(action_section)
		};
		None
	}

	#[inline]
	fn str_as_bool(&self, s: &str) -> bool {
		match s {
			"False" | "false" => false,
			"True"  | "true" => true,
			_ => false,
		}
	}

	#[inline]
	fn decode_finished(&self, mut body: IniAction<&str>, actions: Actions) -> Option<Ini> {
		if body.terminal.is_none() {
			body.terminal = Some(false);
		}
		if let (Some(name), Some(exec), Some(terminal)) = (body.name, body.exec, body.terminal) {
			return Some(Ini { name: name.to_owned(), exec: exec.to_owned(), terminal, actions });
		}
		None
	}

	#[inline]
	fn decode_kv_hash(&self, line: &str, action: &mut IniAction<String>) {
		if let Some((act_field_key, act_field_val)) = line.split_once("=") {
			match act_field_key {
				"Name" => action.name = Some(act_field_val.to_owned()),
				"Exec" => action.exec = Some(act_field_val.to_owned()),
				"Terminal" => action.terminal = Some(self.str_as_bool(act_field_val)),
				_ => ()
			}
		};
	}

	pub fn decode(&self) -> Option<Ini> {
		let ini_lines = self.lines()?;
		let mut body = self.body();

		let mut curr_act_name: Option<&str> = None;
		let mut h_acts: Actions = HashMap::new();

		for line in ini_lines {
			//Check if we ran into a custom desktop action
			if let Some(act) = self.is_action(line) {
				curr_act_name = Some(act);
				h_acts.insert(act.to_owned(), self.body());
				continue;
			}
			//Are we in a desktop action?
			if let Some(act_name) = curr_act_name {
				h_acts.entry(act_name.to_owned()).and_modify(|action| self.decode_kv_hash(line, action));
				continue;
			};

			let (field_key, field_val) = match line.split_once("=") {
				Some(kv) => kv,
				None => continue,
			};
			match field_key {
				"Name" => body.name = Some(field_val),
				"Exec" => body.exec = Some(field_val),
				"Terminal" => body.terminal = Some(self.str_as_bool(field_val)),
				"NoDisplay" => if self.str_as_bool(field_val) { return None; },
				_ => continue,
			}
		}

		self.decode_finished(body, h_acts)
	}
}

pub struct Spawn {
	name: String,
	terminal: Option<String>,
}
impl Spawn {
    pub const fn new(name: String, terminal: Option<String>) -> Self {
    	Self { name, terminal }
    }

    fn sys_exec(&self, app: Ini, stdout: bool) -> Result<(), RunError> {
		let mut args: Vec<String> = app.exec.split_whitespace()
			.filter(|s| !matches!(*s, "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%k" | "%v" | "%m" | "%c" | "%i" | "%s"))
			.map(|s| s.to_owned())
			.collect();
		let std_inherit_or_null = || if stdout { Stdio::inherit() } else { Stdio::null() };

		if app.terminal {
			match self.terminal.clone() {
				Some(term) => {
					args.insert(0, term);
					args.insert(1, "-e".to_owned());
				},
				None => return Err(RunError::NoTerminal(app.name))
			}
		}
		match Command::new(args.remove(0))
			.args(args)
			.stdout(std_inherit_or_null())
			.stderr(std_inherit_or_null())
			.spawn()
		{
			Ok(mut child_proc) => {
				println!("Launching application {:?}.", app.name);
				if stdout {
					child_proc.wait().map_err(RunError::Exec)?;
				}
				Ok(())
			},
			Err(spawn_err) => Err(RunError::Exec(spawn_err))
		}
    }

    pub fn run(&self, stdout: bool) -> Result<(), RunError> {
		let all_apps = Installed.all()?;
		for app_entry in all_apps.into_iter() {
			if app_entry.name.to_lowercase() == self.name.to_lowercase() {
				return self.sys_exec(app_entry, stdout)
			};
		};
		Err(RunError::NotFound(self.name.clone()))
	}
}

pub struct Installed;
impl Installed {
	pub const UNIX_FLATPAK_APPS_PATH: &str = "/var/lib/flatpak/exports/share/applications";
	pub const UNIX_USER_APPS_PATH: &str = ".local/share/applications";
	pub const UNIX_SYS_APPS_PATH: &str = "/usr/share/applications";

	fn to_inis(&self, apps: &[PathBuf]) -> Vec<Ini> {
		apps.iter().filter_map(|app_buf| {
			fs::read(app_buf).ok()
				.and_then(|bytes| String::from_utf8(bytes).ok())
				.and_then(|entry_inner| ApplicationEntry::new(entry_inner).decode())
		}).collect()
	}

	fn get_app_bufs(&self, read_dir: Option<ReadDir>) -> Option<Vec<PathBuf>> {
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

	fn read(&self, path: &str) -> Option<Vec<Ini>> {
		let sys_apps = self.get_app_bufs(fs::read_dir(path).ok())?;
		Some(self.to_inis(&sys_apps))
	}

	pub fn flatpak(&self) -> Option<Vec<Ini>> {
		self.read(Self::UNIX_FLATPAK_APPS_PATH)
	}

	pub fn system(&self) -> Option<Vec<Ini>> {
		self.read(Self::UNIX_SYS_APPS_PATH)
	}

	pub fn user(&self) -> Option<Vec<Ini>> {
		let user_apps = self.get_app_bufs(env::home_dir().map(|mut home| {
			home.push(Self::UNIX_USER_APPS_PATH);
			home
		}).and_then(|user_apps| fs::read_dir(user_apps).ok()))?;
		Some(self.to_inis(&user_apps))
	}

	pub fn all(&self) -> Result<Vec<Ini>, RunError> {
		let mut user_apps = self.user().ok_or(RunError::User)?;
		let mut sys_apps = self.system().ok_or(RunError::System)?;
		let mut flatpak_apps = self.flatpak().ok_or(RunError::Flatpak)?;
		user_apps.append(&mut flatpak_apps);
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
	pub fn actions(&self, actions: Actions) {
		if actions.is_empty() { return; }
		actions.into_iter().for_each(|(name, act)| {
			println!("\n\t[Action]\n\t{name}\n\t- Name={}\n\t- Exec={}\n\t- Terminal={}",
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
