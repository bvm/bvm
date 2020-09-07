use dprint_cli_core::checksums::{parse_checksum_path_or_url, ChecksumPathOrUrl};
use dprint_cli_core::types::ErrBox;

use super::types::BinaryName;

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
    Version,
    Init,
    ClearUrlCache,
    Help(String),
}

pub struct ResolveCommand {
    pub binary_name: String,
}

pub struct UseBinaryCommand {
    pub binary_name: BinaryName,
    pub version: String,
}

pub struct InstallCommand {
    pub use_command: bool,
    pub force: bool,
}

pub struct InstallUrlCommand {
    pub url: ChecksumPathOrUrl,
    pub use_command: bool,
    pub force: bool,
}

pub struct UninstallCommand {
    pub binary_name: BinaryName,
    pub version: String,
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
        if let Some(url) = install_matches.value_of("url").map(String::from) {
            SubCommand::InstallUrl(InstallUrlCommand {
                url: parse_checksum_path_or_url(&url),
                use_command,
                force,
            })
        } else {
            SubCommand::Install(InstallCommand { use_command, force })
        }
    } else if matches.is_present("use") {
        let use_matches = matches.subcommand_matches("use").unwrap();
        if let Some(binary_name) = use_matches.value_of("binary_name").map(String::from) {
            let binary_name = parse_binary_name(binary_name);
            SubCommand::UseBinary(UseBinaryCommand {
                binary_name,
                version: use_matches.value_of("version").map(String::from).unwrap(),
            })
        } else {
            SubCommand::Use
        }
    } else if matches.is_present("uninstall") {
        let uninstall_matches = matches.subcommand_matches("uninstall").unwrap();
        let binary_name = parse_binary_name(uninstall_matches.value_of("binary_name").map(String::from).unwrap());
        SubCommand::Uninstall(UninstallCommand {
            binary_name,
            version: uninstall_matches.value_of("version").map(String::from).unwrap(),
        })
    } else if matches.is_present("list") {
        SubCommand::List
    } else if matches.is_present("init") {
        SubCommand::Init
    } else if matches.is_present("clear-url-cache") {
        SubCommand::ClearUrlCache
    } else {
        SubCommand::Help({
            let mut text = Vec::new();
            cli_parser.write_help(&mut text).unwrap();
            String::from_utf8(text).unwrap()
        })
    };

    Ok(CliArgs { sub_command })
}

fn parse_binary_name(text: String) -> BinaryName {
    let index = text.find('/');
    if let Some(index) = index {
        let owner_name = text[0..index].to_string();
        let name = text[index + 1..].to_string();
        BinaryName {
            owner: Some(owner_name),
            name,
        }
    } else {
        BinaryName {
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
                .about("Installs the binaries for the current configuration file.")
                .arg(
                    Arg::with_name("url")
                        .help("The url of the binary manifest to install.")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("use")
                        .help("Use the installed binary or binaries on the path.")
                        .long("use")
                        .takes_value(false),
                )
                .arg(
                    Arg::with_name("force")
                        .help("Reinstall the binary if it is already installed.")
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
        .arg(
            Arg::with_name("version")
                .short("v")
                .long("version")
                .help("Prints the version.")
                .takes_value(false),
        )
}
