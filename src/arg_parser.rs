use url::Url;

use dprint_cli_core::checksums::{parse_checksum_path_or_url, ChecksumPathOrUrl};
use dprint_cli_core::types::ErrBox;

use super::types::{BinarySelector, CommandName, PathOrVersionSelector, Version};

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
    Version,
    Init,
    ClearUrlCache,
    Help(String),
}

pub struct ResolveCommand {
    pub binary_name: String,
}

pub struct UseBinaryCommand {
    pub selector: BinarySelector,
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
    pub selector: BinarySelector,
    pub version: Option<String>,
}

pub struct UninstallCommand {
    pub selector: BinarySelector,
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

pub fn parse_args(args: Vec<String>) -> Result<CliArgs, ErrBox> {
    let mut cli_parser = create_cli_parser();
    let matches = match cli_parser.get_matches_from_safe_borrow(args) {
        Ok(result) => result,
        Err(err) => return err!("{}", err.to_string()),
    };

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
                let selector = parse_binary_selector(url_or_name);
                SubCommand::InstallUrl(InstallUrlCommand {
                    url_or_name: UrlOrName::Name(InstallName { selector, version }),
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
            let selector = parse_binary_selector(binary_name);
            SubCommand::UseBinary(UseBinaryCommand {
                selector,
                version: PathOrVersionSelector::parse(&use_matches.value_of("version").map(String::from).unwrap())?,
            })
        } else {
            SubCommand::Use
        }
    } else if matches.is_present("uninstall") {
        let uninstall_matches = matches.subcommand_matches("uninstall").unwrap();
        let selector = parse_binary_selector(uninstall_matches.value_of("binary_name").map(String::from).unwrap());
        SubCommand::Uninstall(UninstallCommand {
            selector,
            version: Version::parse(&uninstall_matches.value_of("version").map(String::from).unwrap())?,
        })
    } else if matches.is_present("list") {
        SubCommand::List
    } else if matches.is_present("init") {
        SubCommand::Init
    } else if matches.is_present("clear-url-cache") {
        SubCommand::ClearUrlCache
    } else if matches.is_present("registry") {
        let registry_matches = matches.subcommand_matches("registry").unwrap();
        if registry_matches.is_present("add") {
            let add_matches = registry_matches.subcommand_matches("add").unwrap();
            SubCommand::Registry(RegistrySubCommand::Add(RegistryAddCommand {
                url: add_matches.value_of("url").map(String::from).unwrap(),
            }))
        } else if registry_matches.is_present("remove") {
            let remove_matches = registry_matches.subcommand_matches("remove").unwrap();
            SubCommand::Registry(RegistrySubCommand::Remove(RegistryRemoveCommand {
                url: remove_matches.value_of("url").map(String::from).unwrap(),
            }))
        } else if registry_matches.is_present("list") {
            SubCommand::Registry(RegistrySubCommand::List)
        } else {
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

fn parse_binary_selector(text: String) -> BinarySelector {
    let index = text.find('/');
    if let Some(index) = index {
        let owner_name = text[0..index].to_string();
        let name = text[index + 1..].to_string();
        BinarySelector {
            owner: Some(owner_name),
            name: CommandName::from_string(name),
        }
    } else {
        BinarySelector {
            owner: None,
            name: CommandName::from_string(text),
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
                        .requires("version"),
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
        .arg(
            Arg::with_name("version")
                .short("v")
                .long("version")
                .help("Prints the version.")
                .takes_value(false),
        )
}
