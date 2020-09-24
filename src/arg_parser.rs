use url::Url;

use dprint_cli_core::checksums::{parse_checksum_path_or_url, ChecksumPathOrUrl};
use dprint_cli_core::types::ErrBox;

use super::types::{NameSelector, PathOrVersionSelector, Version, VersionSelector};

pub struct CliArgs {
    pub sub_command: SubCommand,
}

pub enum SubCommand {
    Resolve(ResolveCommand),
    Use,
    UseBinary(UseBinaryCommand),
    List,
    Install(InstallCommand),
    InstallUrl(InstallUrlCommand),
    Uninstall(UninstallCommand),
    Registry(RegistrySubCommand),
    Add(AddCommand),
    Version,
    Init,
    ClearUrlCache,
    Shell(ShellSubCommand),
    Help(String),
}

pub struct ResolveCommand {
    pub binary_name: String,
}

pub struct UseBinaryCommand {
    pub name_selector: NameSelector,
    pub version: PathOrVersionSelector,
}

pub struct InstallCommand {
    pub use_command: bool,
    pub force: bool,
}

pub struct InstallUrlCommand {
    pub url_or_name: UrlOrName,
    pub use_command: bool,
    pub force: bool,
}

pub enum UrlOrName {
    Url(ChecksumPathOrUrl),
    Name(InstallName),
}

pub struct InstallName {
    pub name_selector: NameSelector,
    pub version_selector: Option<VersionSelector>,
}

pub struct UninstallCommand {
    pub name_selector: NameSelector,
    pub version: Version,
}

pub enum RegistrySubCommand {
    Add(RegistryAddCommand),
    Remove(RegistryRemoveCommand),
    List,
}

pub struct RegistryAddCommand {
    pub url: String,
}

pub struct RegistryRemoveCommand {
    pub url: String,
}

pub struct AddCommand {
    pub url_or_name: UrlOrName,
}

pub enum ShellSubCommand {
    GetNewPath(ShellGetNewPathCommand),
    ClearPendingChanges,
    GetPaths,
    #[cfg(target_os = "windows")]
    WindowsInstall(ShellWindowsInstallCommand),
    #[cfg(target_os = "windows")]
    WindowsUninstall(ShellWindowsUninstallCommand),
}

pub struct ShellGetNewPathCommand {
    pub current_sys_path: String,
}

#[cfg(target_os = "windows")]
pub struct ShellWindowsInstallCommand {
    pub install_path: String,
}

#[cfg(target_os = "windows")]
pub struct ShellWindowsUninstallCommand {
    pub install_path: String,
}

pub fn parse_args(args: Vec<String>) -> Result<CliArgs, ErrBox> {
    let mut cli_parser = create_cli_parser();
    let matches = match cli_parser.get_matches_from_safe_borrow(args) {
        Ok(result) => result,
        Err(err) => return err!("{}", err.to_string()),
    };

    // todo: use a match statement
    let sub_command = if matches.is_present("resolve") {
        let resolve_matches = matches.subcommand_matches("resolve").unwrap();
        SubCommand::Resolve(ResolveCommand {
            binary_name: resolve_matches.value_of("binary_name").map(String::from).unwrap(),
        })
    } else if matches.is_present("version") {
        SubCommand::Version
    } else if matches.is_present("install") {
        let install_matches = matches.subcommand_matches("install").unwrap();
        let use_command = install_matches.is_present("use");
        let force = install_matches.is_present("force");
        if let Some(url_or_name) = install_matches.value_of("url_or_name").map(String::from) {
            let version = install_matches.value_of("version").map(String::from);
            if version.is_some() || Url::parse(&url_or_name).is_err() {
                let name_selector = parse_name_selector(url_or_name);
                SubCommand::InstallUrl(InstallUrlCommand {
                    url_or_name: UrlOrName::Name(InstallName {
                        name_selector,
                        version_selector: if let Some(v) = &version {
                            Some(VersionSelector::parse(v)?)
                        } else {
                            None
                        },
                    }),
                    use_command,
                    force,
                })
            } else {
                SubCommand::InstallUrl(InstallUrlCommand {
                    url_or_name: UrlOrName::Url(parse_checksum_path_or_url(&url_or_name)),
                    use_command,
                    force,
                })
            }
        } else {
            SubCommand::Install(InstallCommand { use_command, force })
        }
    } else if matches.is_present("use") {
        let use_matches = matches.subcommand_matches("use").unwrap();
        if let Some(binary_name) = use_matches.value_of("binary_name").map(String::from) {
            let name_selector = parse_name_selector(binary_name);
            SubCommand::UseBinary(UseBinaryCommand {
                name_selector,
                version: PathOrVersionSelector::parse(
                    &use_matches
                        .value_of("version")
                        .map(String::from)
                        .unwrap_or("*".to_string()),
                )?,
            })
        } else {
            SubCommand::Use
        }
    } else if matches.is_present("uninstall") {
        let uninstall_matches = matches.subcommand_matches("uninstall").unwrap();
        let name_selector = parse_name_selector(uninstall_matches.value_of("binary_name").map(String::from).unwrap());
        SubCommand::Uninstall(UninstallCommand {
            name_selector,
            version: Version::parse(&uninstall_matches.value_of("version").map(String::from).unwrap())?,
        })
    } else if matches.is_present("list") {
        SubCommand::List
    } else if matches.is_present("init") {
        SubCommand::Init
    } else if matches.is_present("clear-url-cache") {
        SubCommand::ClearUrlCache
    } else if matches.is_present("registry") {
        let registry_sub_command = matches.subcommand_matches("registry").unwrap();
        match registry_sub_command.subcommand() {
            ("add", Some(matches)) => SubCommand::Registry(RegistrySubCommand::Add(RegistryAddCommand {
                url: matches.value_of("url").map(String::from).unwrap(),
            })),
            ("remove", Some(matches)) => SubCommand::Registry(RegistrySubCommand::Remove(RegistryRemoveCommand {
                url: matches.value_of("url").map(String::from).unwrap(),
            })),
            ("list", _) => SubCommand::Registry(RegistrySubCommand::List),
            _ => unreachable!(),
        }
    } else if matches.is_present("add") {
        let matches = matches.subcommand_matches("add").unwrap();
        let url_or_name = matches.value_of("url_or_name").map(String::from).unwrap();
        let version = matches.value_of("version").map(String::from);
        if version.is_some() || Url::parse(&url_or_name).is_err() {
            let name_selector = parse_name_selector(url_or_name);
            SubCommand::Add(AddCommand {
                url_or_name: UrlOrName::Name(InstallName {
                    name_selector,
                    version_selector: if let Some(v) = &version {
                        Some(VersionSelector::parse(v)?)
                    } else {
                        None
                    },
                }),
            })
        } else {
            SubCommand::Add(AddCommand {
                url_or_name: UrlOrName::Url(parse_checksum_path_or_url(&url_or_name)),
            })
        }
    } else if matches.is_present("hidden-shell") {
        let matches = matches.subcommand_matches("hidden-shell").unwrap();
        if matches.is_present("get-new-path") {
            let matches = matches.subcommand_matches("get-new-path").unwrap();
            SubCommand::Shell(ShellSubCommand::GetNewPath(ShellGetNewPathCommand {
                current_sys_path: matches.value_of("current-sys-path").map(String::from).unwrap(),
            }))
        } else if matches.is_present("clear-pending-changes") {
            SubCommand::Shell(ShellSubCommand::ClearPendingChanges)
        } else if matches.is_present("get-paths") {
            SubCommand::Shell(ShellSubCommand::GetPaths)
        } else {
            #[cfg(target_os = "windows")]
            if matches.is_present("windows-install") {
                let matches = matches.subcommand_matches("windows-install").unwrap();
                SubCommand::Shell(ShellSubCommand::WindowsInstall(ShellWindowsInstallCommand {
                    install_path: matches.value_of("install-dir").map(String::from).unwrap(),
                }))
            } else if matches.is_present("windows-uninstall") {
                let matches = matches.subcommand_matches("windows-uninstall").unwrap();
                SubCommand::Shell(ShellSubCommand::WindowsUninstall(ShellWindowsUninstallCommand {
                    install_path: matches.value_of("install-dir").map(String::from).unwrap(),
                }))
            } else {
                unreachable!();
            }
            #[cfg(unix)]
            unreachable!();
        }
    } else {
        SubCommand::Help({
            let mut text = Vec::new();
            cli_parser.write_help(&mut text).unwrap();
            String::from_utf8(text).unwrap()
        })
    };

    Ok(CliArgs { sub_command })
}

fn parse_name_selector(text: String) -> NameSelector {
    let index = text.find('/');
    if let Some(index) = index {
        let owner_name = text[0..index].to_string();
        let name = text[index + 1..].to_string();
        NameSelector {
            owner: Some(owner_name),
            name,
        }
    } else {
        NameSelector {
            owner: None,
            name: text,
        }
    }
}

fn create_cli_parser<'a, 'b>() -> clap::App<'a, 'b> {
    use clap::{App, AppSettings, Arg, SubCommand};
    App::new("bvm")
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::DeriveDisplayOrder)
        .bin_name("bvm")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Copyright 2020 by David Sherret")
        .about("Runs versions of specific binaries based on the current working directory.")
        .usage("bvm <SUBCOMMAND> [OPTIONS]")
        .template(
            r#"{bin} {version}
{author}

{about}

USAGE:
    {usage}

SUBCOMMANDS:
{subcommands}

OPTIONS:
{unified}

ARGS:
{positionals}

{after-help}"#,
        )
        .after_help(r#"TODO: Will fill in this info later..."#)
        .subcommand(
            SubCommand::with_name("install")
                .about("Installs the binaries for the current configuration file when no arguments or installs based on the provided arguments.")
                .arg(
                    Arg::with_name("url_or_name")
                        .help("The url of the binary manifest to install or the name if also providing a version.")
                        .takes_value(true)
                        .conflicts_with("name"),
                )
                .arg(
                    Arg::with_name("version")
                        .help("The version of the binary to install.")
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("use")
                        .help("Use the installed binary/binaries on the path.")
                        .long("use")
                        .takes_value(false),
                )
                .arg(
                    Arg::with_name("force")
                        .help("Reinstall the binary/binaries if it is already installed.")
                        .long("force")
                        .takes_value(false),
                ),
        )
        .subcommand(
            SubCommand::with_name("uninstall")
                .about("Uninstalls the specified binary version.")
                .arg(
                    Arg::with_name("binary_name")
                        .help("The binary name.")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("version")
                        .help("The version of the binary to uninstall.")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("use")
                .about("Select a different version to use globally of a binary. Specify no arguments to use the binaries in the current configuration file.")
                .arg(
                    Arg::with_name("binary_name")
                        .help("The binary name.")
                        .takes_value(true)
                )
                .arg(
                    Arg::with_name("version")
                        .help("The version of the binary to use or 'path' to use the binary on the path.")
                        .takes_value(true),
                ),
        )
        .subcommand(SubCommand::with_name("list").about("Output a list of installed binary versions."))
        .subcommand(SubCommand::with_name("init").about("Creates an empty .bvmrc.json file in the current directory."))
        .subcommand(
            SubCommand::with_name("add")
                .about("Programmatically adds a binary to the .bvmrc.json file.")
                .arg(
                    Arg::with_name("url_or_name")
                        .help("The url or name of the binary.")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("version")
                        .help("The version to add if providing a name.")
                        .required(false),
                )
        )
        .subcommand(
            SubCommand::with_name("resolve")
                .about("Outputs the binary path according to the current working directory.")
                .arg(
                    Arg::with_name("binary_name")
                        .help("The binary name to resolve.")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(SubCommand::with_name("clear-url-cache").about("Clears the cache of downloaded urls. Does not remove any installed binaries."))
        .subcommand(
            SubCommand::with_name("registry")
                .about("Commands related to storing urls to binary version registries.")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("add")
                        .about("Add a url to a registry.")
                        .arg(
                            Arg::with_name("url")
                                .help("The url of the binary registry.")
                                .takes_value(true)
                                .required(true)
                        )
                )
                .subcommand(
                    SubCommand::with_name("remove")
                        .about("Remove a url from the registry.")
                        .arg(
                            Arg::with_name("url")
                                .help("The url of the binary registry.")
                                .takes_value(true)
                                .required(true)
                        )
                )
                .subcommand(
                    SubCommand::with_name("list")
                        .about("List all the urls in the registry.")
                )
        )
        .subcommand(
            SubCommand::with_name("util")
                .about("Commands that can be used for setting up the system.")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("command-exists")
                        .about("Exits with code 0 when the provided command is on the path or locally installed in bvm and 1 when not.")
                        .arg(
                            Arg::with_name("full-binary-name")
                                .help("The full binary name (owner/name).")
                                .takes_value(true)
                                .required(true)
                        )
                        .arg(
                            Arg::with_name("command")
                                .help("The name of the command.")
                                .takes_value(true)
                                .required(true)
                        )
                )
        )
        .subcommand(
            SubCommand::with_name("hidden-shell")
                .setting(AppSettings::Hidden)
                .subcommand(
                    SubCommand::with_name("get-new-path")
                        .arg(
                            Arg::with_name("current-sys-path")
                                .takes_value(true)
                                .required(true)
                        )
                )
                .subcommand(
                    SubCommand::with_name("clear-pending-changes")
                )
                .subcommand(
                    SubCommand::with_name("get-paths")
                )
                .subcommand(
                    SubCommand::with_name("windows-install")
                        .arg(
                            Arg::with_name("install-dir")
                                .takes_value(true)
                                .required(true)
                        )
                )
                .subcommand(
                    SubCommand::with_name("windows-uninstall")
                        .arg(
                            Arg::with_name("install-dir")
                                .takes_value(true)
                                .required(true)
                        )
                )
        )
        .arg(
            Arg::with_name("version")
                .short("v")
                .long("version")
                .help("Prints the version.")
                .takes_value(false),
        )
}
