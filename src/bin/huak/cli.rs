use crate::error::{CliResult, Error};
use clap::{Command, CommandFactory, Parser, Subcommand};
use clap_complete::{self, Shell};
use huak::{
    ops::{self, find_workspace, OperationConfig},
    BuildOptions, CleanOptions, Error as HuakError, FormatOptions, HuakResult,
    InstallerOptions, LintOptions, PublishOptions, TerminalOptions,
    TestOptions, Verbosity, WorkspaceOptions,
};
use pep440_rs::Version;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

/// A Python package manager written in Rust inspired by Cargo.
#[derive(Parser)]
#[command(version, author, about, arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, global = true)]
    quiet: bool,
}

// List of commands.
#[derive(Subcommand)]
#[clap(rename_all = "kebab-case")]
enum Commands {
    /// Activate the virtual envionrment.
    Activate,
    /// Add dependencies to the project.
    Add {
        #[arg(num_args = 1.., required = true)]
        dependencies: Vec<Dependency>,
        /// Adds an optional dependency group.
        #[arg(long)]
        group: Option<String>,
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Build tarball and wheel for the project.
    Build {
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Generates a shell completion script for supported shells.
    Completion {
        #[arg(short, long, value_name = "shell")]
        shell: Option<Shell>,
        #[arg(short, long)]
        /// Installs the completion script in your shell init file.
        /// If this flag is passed the --shell is required
        install: bool,
        #[arg(short, long)]
        /// Uninstalls the completion script from your shell init file.
        /// If this flag is passed the --shell is required
        uninstall: bool,
    },
    /// Remove tarball and wheel from the built project.
    Clean {
        #[arg(long, required = false)]
        /// Remove all .pyc files.
        include_pyc: bool,
        #[arg(long, required = false)]
        /// Remove all __pycache__ directories.
        include_pycache: bool,
    },
    /// Auto-fix fixable lint conflicts
    Fix {
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Format the project's Python code.
    Fmt {
        /// Check if Python code is formatted.
        #[arg(long)]
        check: bool,
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Initialize the existing project.
    Init {
        /// Use a application template [default].
        #[arg(long, conflicts_with = "lib")]
        app: bool,
        /// Use a library template.
        #[arg(long, conflicts_with = "app")]
        lib: bool,
        /// Don't initialize VCS in the project
        #[arg(long)]
        no_vcs: bool,
    },
    /// Install the dependencies of an existing project.
    Install {
        /// Install optional dependency groups
        #[arg(long, num_args = 1..)]
        groups: Option<Vec<String>>,
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Lint the project's Python code.
    Lint {
        /// Address any fixable lints.
        #[arg(long)]
        fix: bool,
        /// Perform type-checking.
        #[arg(long)]
        no_types: bool,
        /// Pass trailing arguments with `--` to `ruff`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Create a new project at <path>.
    New {
        /// Use a application template [default].
        #[arg(long, conflicts_with = "lib")]
        app: bool,
        /// Use a library template.
        #[arg(long, conflicts_with = "app")]
        lib: bool,
        /// Path and name of the python package
        path: String,
        /// Don't initialize VCS in the new project
        #[arg(long)]
        no_vcs: bool,
    },
    /// Builds and uploads current project to a registry.
    Publish {
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Manage python installations.
    Python {
        #[command(subcommand)]
        command: Python,
    },
    /// Remove dependencies from the project.
    Remove {
        #[arg(num_args = 1.., required = true)]
        dependencies: Vec<String>,
        /// Remove from optional dependency group
        #[arg(long, num_args = 1)]
        group: Option<String>,
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Run a command within the project's environment context.
    Run {
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },
    /// Test the project's Python code.
    Test {
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Update the project's dependencies.
    Update {
        #[arg(num_args = 0..)]
        dependencies: Option<Vec<String>>,
        /// Update an optional dependency group
        #[arg(long)]
        group: Option<String>,
        /// Pass trailing arguments with `--`.
        #[arg(last = true)]
        trailing: Option<Vec<String>>,
    },
    /// Display the version of the project.
    Version,
}

#[derive(Subcommand)]
enum Python {
    /// List the installed Python interpreters.
    List,
    /// Use a specific Python interpreter.
    Use {
        /// A Python interpreter version number.
        #[arg(required = true)]
        version: PythonVersion,
    },
}

// Command gating for Huak.
impl Cli {
    pub fn run(self) -> CliResult<()> {
        let workspace_root =
            find_workspace().unwrap_or(std::env::current_dir()?);
        let verbosity = match self.quiet {
            true => Verbosity::Quiet,
            false => Verbosity::Normal,
        };
        let mut operation_config = OperationConfig {
            workspace_root,
            terminal_options: TerminalOptions { verbosity },
            ..Default::default()
        };
        match self.command {
            Commands::Activate => activate(operation_config),
            Commands::Add {
                dependencies,
                group,
                trailing,
            } => {
                operation_config.installer_options =
                    Some(InstallerOptions { args: trailing });
                add(dependencies, group, operation_config)
            }
            Commands::Build { trailing } => {
                operation_config.build_options =
                    Some(BuildOptions { args: trailing });
                build(operation_config)
            }
            Commands::Clean {
                include_pyc,
                include_pycache,
            } => {
                let options = CleanOptions {
                    include_pycache,
                    include_compiled_bytecode: include_pyc,
                };
                operation_config.clean_options = Some(options);
                clean(operation_config)
            }
            Commands::Completion {
                shell,
                install,
                uninstall,
            } => {
                if (install || uninstall) && shell.is_none() {
                    Err(HuakError::HuakConfigurationError(
                        "no shell provided".to_string(),
                    ))
                } else if install {
                    run_with_install(shell)
                } else if uninstall {
                    run_with_uninstall(shell)
                } else {
                    generate_shell_completion_script();
                    Ok(())
                }
            }
            Commands::Fix { trailing } => {
                operation_config.lint_options = Some(LintOptions {
                    args: trailing,
                    include_types: false,
                });
                if let Some(options) = operation_config.lint_options.as_mut() {
                    if let Some(args) = options.args.as_mut() {
                        args.push("--fix".to_string());
                    }
                }
                fix(operation_config)
            }
            Commands::Fmt { check, trailing } => {
                operation_config.format_options =
                    Some(FormatOptions { args: trailing });
                if check {
                    if let Some(options) =
                        operation_config.format_options.as_mut()
                    {
                        if let Some(args) = options.args.as_mut() {
                            args.push("--check".to_string());
                        } else {
                            options.args = Some(vec!["--check".to_string()]);
                        }
                    }
                }
                fmt(operation_config)
            }
            Commands::Init { app, lib, no_vcs } => {
                operation_config.workspace_root = std::env::current_dir()?;
                operation_config.workspace_options =
                    Some(WorkspaceOptions { uses_git: !no_vcs });
                init(app, lib, operation_config)
            }
            Commands::Install { groups, trailing } => {
                operation_config.installer_options =
                    Some(InstallerOptions { args: trailing });
                install(groups, operation_config)
            }
            Commands::Lint {
                fix,
                no_types,
                trailing,
            } => {
                operation_config.lint_options = Some(LintOptions {
                    args: trailing,
                    include_types: !no_types,
                });
                if fix {
                    if let Some(options) =
                        operation_config.lint_options.as_mut()
                    {
                        if let Some(args) = options.args.as_mut() {
                            args.push("--fix".to_string());
                        }
                    }
                }
                lint(operation_config)
            }
            Commands::New {
                path,
                app,
                lib,
                no_vcs,
            } => {
                operation_config.workspace_root = PathBuf::from(path);
                operation_config.workspace_options =
                    Some(WorkspaceOptions { uses_git: !no_vcs });
                new(app, lib, operation_config)
            }
            Commands::Publish { trailing } => {
                operation_config.publish_options =
                    Some(PublishOptions { args: trailing });
                publish(operation_config)
            }
            Commands::Python { command } => python(command, operation_config),
            Commands::Remove {
                dependencies,
                group,
                trailing,
            } => {
                operation_config.installer_options =
                    Some(InstallerOptions { args: trailing });
                remove(dependencies, group, operation_config)
            }
            Commands::Run { command } => run(command, operation_config),
            Commands::Test { trailing } => {
                operation_config.test_options =
                    Some(TestOptions { args: trailing });
                test(operation_config)
            }
            Commands::Update {
                dependencies,
                group,
                trailing,
            } => {
                operation_config.installer_options =
                    Some(InstallerOptions { args: trailing });
                update(dependencies, group, operation_config)
            }
            Commands::Version => version(operation_config),
        }
        .map_err(|e| Error::new(e, ExitCode::FAILURE))
    }
}

fn activate(operation_config: OperationConfig) -> HuakResult<()> {
    ops::activate_venv(&operation_config)
}

fn add(
    dependencies: Vec<Dependency>,
    group: Option<String>,
    operation_config: OperationConfig,
) -> HuakResult<()> {
    let deps = dependencies
        .iter()
        .map(|item| item.to_string())
        .collect::<Vec<String>>();
    match group.as_ref() {
        Some(it) => {
            ops::add_project_optional_dependencies(&deps, it, &operation_config)
        }
        None => ops::add_project_dependencies(&deps, &operation_config),
    }
}

fn build(operation_config: OperationConfig) -> HuakResult<()> {
    ops::build_project(&operation_config)
}

fn clean(operation_config: OperationConfig) -> HuakResult<()> {
    ops::clean_project(&operation_config)
}

fn fix(operation_config: OperationConfig) -> HuakResult<()> {
    ops::lint_project(&operation_config)
}

fn fmt(operation_config: OperationConfig) -> HuakResult<()> {
    ops::format_project(&operation_config)
}

fn init(
    app: bool,
    _lib: bool,
    operation_config: OperationConfig,
) -> HuakResult<()> {
    if app {
        ops::init_app_project(&operation_config)
    } else {
        ops::init_lib_project(&operation_config)
    }
}

fn install(
    groups: Option<Vec<String>>,
    operation_config: OperationConfig,
) -> HuakResult<()> {
    if let Some(it) = groups {
        ops::install_project_optional_dependencies(&it, &operation_config)
    } else {
        ops::install_project_dependencies(&operation_config)
    }
}

fn lint(operation_config: OperationConfig) -> HuakResult<()> {
    ops::lint_project(&operation_config)
}

fn new(
    app: bool,
    _lib: bool,
    operation_config: OperationConfig,
) -> HuakResult<()> {
    if app {
        ops::new_app_project(&operation_config)
    } else {
        ops::new_lib_project(&operation_config)
    }
}

fn publish(operation_config: OperationConfig) -> HuakResult<()> {
    ops::publish_project(&operation_config)
}

fn python(
    command: Python,
    operation_config: OperationConfig,
) -> HuakResult<()> {
    match command {
        Python::List => ops::list_python(&operation_config),
        Python::Use { version } => {
            ops::use_python(version.0, &operation_config)
        }
    }
}

fn remove(
    dependencies: Vec<String>,
    group: Option<String>,
    operation_config: OperationConfig,
) -> HuakResult<()> {
    match group.as_ref() {
        Some(it) => ops::remove_project_optional_dependencies(
            &dependencies,
            it,
            &operation_config,
        ),
        None => {
            ops::remove_project_dependencies(&dependencies, &operation_config)
        }
    }
}

fn run(
    command: Vec<String>,
    operation_config: OperationConfig,
) -> HuakResult<()> {
    ops::run_command_str(&command.join(" "), &operation_config)
}

fn test(operation_config: OperationConfig) -> HuakResult<()> {
    ops::test_project(&operation_config)
}

fn update(
    dependencies: Option<Vec<String>>,
    groups: Option<String>,
    operation_config: OperationConfig,
) -> HuakResult<()> {
    match groups.as_ref() {
        Some(it) => ops::update_project_optional_dependencies(
            dependencies,
            it,
            &operation_config,
        ),
        None => {
            ops::update_project_dependencies(dependencies, &operation_config)
        }
    }
}

fn version(operation_config: OperationConfig) -> HuakResult<()> {
    ops::display_project_version(&operation_config)
}

fn generate_shell_completion_script() {
    let mut cmd = Cli::command();
    clap_complete::generate(
        Shell::Bash,
        &mut cmd,
        "huak",
        &mut std::io::stdout(),
    )
}

fn run_with_install(shell: Option<Shell>) -> HuakResult<()> {
    let sh = match shell {
        Some(it) => it,
        None => {
            return Err(HuakError::HuakConfigurationError(
                "no shell provided".to_string(),
            ))
        }
    };
    let mut cmd = Cli::command();
    match sh {
        Shell::Bash => add_completion_bash(),
        Shell::Elvish => Err(HuakError::UnimplementedError(
            "elvish completion".to_string(),
        )),
        Shell::Fish => add_completion_fish(&mut cmd),
        Shell::PowerShell => Err(HuakError::UnimplementedError(
            "powershell completion".to_string(),
        )),
        Shell::Zsh => add_completion_zsh(&mut cmd),
        _ => Err(HuakError::HuakConfigurationError(
            "invalid shell".to_string(),
        )),
    }
}

fn run_with_uninstall(shell: Option<Shell>) -> HuakResult<()> {
    let sh = match shell {
        Some(it) => it,
        None => {
            return Err(HuakError::HuakConfigurationError(
                "no shell provided".to_string(),
            ))
        }
    };
    match sh {
        Shell::Bash => remove_completion_bash(),
        Shell::Elvish => Err(HuakError::UnimplementedError(
            "elvish completion".to_string(),
        )),
        Shell::Fish => remove_completion_fish(),
        Shell::PowerShell => Err(HuakError::UnimplementedError(
            "Powershell completion".to_string(),
        )),
        Shell::Zsh => remove_completion_zsh(),
        _ => Err(HuakError::HuakConfigurationError(
            "invalid shell".to_string(),
        )),
    }
}

/// Bash has a couple of files that can contain the actual completion script.
/// Only the line `eval "$(huak config completion bash)"` needs to be added
/// These files are loaded in the following order:
/// ~/.bash_profile
/// ~/.bash_login
/// ~/.profile
/// ~/.bashrc
pub fn add_completion_bash() -> HuakResult<()> {
    let home = std::env::var("HOME")?;
    let file_path = format!("{home}/.bashrc");
    // Opening file in append mode
    let mut file = File::options().append(true).open(file_path)?;
    // This needs to be a string since there will be a \n prepended if it is
    file.write_all(
        format!(r##"{}eval "$(huak config completion)"{}"##, '\n', '\n')
            .as_bytes(),
    )
    .map_err(HuakError::IOError)
}

/// huak config completion fish > ~/.config/fish/completions/huak.fish
/// Fish has a completions directory in which all files are loaded on init.
/// The naming convention is $HOME/.config/fish/completions/huak.fish
pub fn add_completion_fish(cli: &mut Command) -> HuakResult<()> {
    let home = std::env::var("HOME")?;
    let target_file = format!("{home}/.config/fish/completions/huak.fish");
    generate_target_file(target_file, cli)
}

/// Zsh and fish are the same in the sense that the use an entire directory to collect shell init
/// scripts.
pub fn add_completion_zsh(cli: &mut Command) -> HuakResult<()> {
    let target_file = "/usr/local/share/zsh/site-functions/_huak".to_string();
    generate_target_file(target_file, cli)
}

/// Reads the entire file and removes lines that match exactly with:
/// \neval "$(huak config completion)
pub fn remove_completion_bash() -> HuakResult<()> {
    let home = std::env::var("HOME")?;
    let file_path = format!("{home}/.bashrc");
    let file_content = std::fs::read_to_string(&file_path)?;
    let new_content = file_content.replace(
        &format!(r##"{}eval "$(huak config completion)"{}"##, '\n', '\n'),
        "",
    );
    std::fs::write(&file_path, new_content).map_err(HuakError::IOError)
}

pub fn remove_completion_fish() -> HuakResult<()> {
    let home = std::env::var("HOME")?;
    let target_file = format!("{home}/.config/fish/completions/huak.fish");
    std::fs::remove_file(target_file).map_err(HuakError::IOError)
}

pub fn remove_completion_zsh() -> HuakResult<()> {
    let target_file = "/usr/local/share/zsh/site-functions/_huak".to_string();
    std::fs::remove_file(target_file).map_err(HuakError::IOError)
}

fn generate_target_file<P>(target_file: P, cmd: &mut Command) -> HuakResult<()>
where
    P: AsRef<Path>,
{
    let mut file = File::create(&target_file)?;
    clap_complete::generate(Shell::Fish, cmd, "huak", &mut file);
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Dependency(String);

impl FromStr for Dependency {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.replace('@', "==")))
    }
}

impl ToString for Dependency {
    fn to_string(&self) -> String {
        self.0.to_owned()
    }
}

#[derive(Debug, Clone)]
pub struct PythonVersion(String);

impl FromStr for PythonVersion {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let version = Version::from_str(s).map_err(|_| {
            Error::new(
                HuakError::InternalError("failed to parse version".to_string()),
                ExitCode::FAILURE,
            )
        })?;
        if version.release.len() > 2 {
            return Err(Error::new(
                HuakError::InternalError(format!(
                    "{s} is invalid, use major.minor"
                )),
                ExitCode::FAILURE,
            ));
        }
        Ok(Self(version.to_string()))
    }
}
