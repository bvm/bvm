use dprint_cli_core::checksums::verify_sha256_checksum;
use dprint_cli_core::types::ErrBox;
use std::path::PathBuf;

use super::create_shim;
use crate::environment::Environment;
use crate::plugins::{
    get_plugin_dir, BinaryEnvironment, BinaryIdentifier, BinaryManifestItem, BinaryManifestItemCommand,
    BinaryManifestItemSource, PlatformInfo, PlatformInfoCommand, SerializedPluginFile,
};
use crate::types::{BinaryName, Version};
use crate::utils;

pub struct PluginFile {
    // todo: move these two properties down into PluginFile
    pub url: String,
    pub checksum: String,

    pub(super) file: SerializedPluginFile,
}

pub enum DownloadType {
    Zip,
    Binary,
    TarGz,
}

impl PluginFile {
    pub fn display(&self) -> String {
        format!("{}/{} {}", self.file.owner, self.file.name, self.file.version)
    }

    pub fn get_binary_name(&self) -> BinaryName {
        BinaryName::new(self.file.owner.clone(), self.file.name.clone())
    }

    pub fn version(&self) -> &Version {
        &self.file.version
    }

    pub fn get_url(&self) -> Result<&String, ErrBox> {
        Ok(&self.get_platform_info()?.path)
    }

    pub fn get_url_checksum(&self) -> Result<&String, ErrBox> {
        Ok(&self.get_platform_info()?.checksum)
    }

    pub fn get_commands(&self) -> Result<&Vec<PlatformInfoCommand>, ErrBox> {
        Ok(&self.get_platform_info()?.commands)
    }

    pub fn get_environment(&self) -> Result<&Option<BinaryEnvironment>, ErrBox> {
        Ok(&self.get_platform_info()?.environment)
    }

    pub fn get_download_type(&self) -> Result<DownloadType, ErrBox> {
        let download_type = self.get_platform_info()?.download_type.to_lowercase();
        Ok(match download_type.as_str() {
            "zip" => DownloadType::Zip,
            "binary" => DownloadType::Binary,
            "tar.gz" => DownloadType::TarGz,
            _ => return err!("Unknown download type: {}", download_type),
        })
    }

    pub fn get_output_dir(&self) -> Result<&Option<String>, ErrBox> {
        Ok(&self.get_platform_info()?.output_dir)
    }

    pub fn get_pre_install_command(&self) -> Result<&Option<String>, ErrBox> {
        Ok(&self.get_platform_info()?.on_pre_install)
    }

    pub fn get_post_install_command(&self) -> Result<&Option<String>, ErrBox> {
        Ok(&self.get_platform_info()?.on_post_install)
    }

    fn get_platform_info(&self) -> Result<&PlatformInfo, ErrBox> {
        // todo: how to throw a nice compile error here for an unsupported OS?
        #[cfg(target_os = "linux")]
        return get_plugin_platform_info(&self.file.linux);

        #[cfg(target_os = "macos")]
        return get_plugin_platform_info(&self.file.mac);

        #[cfg(target_os = "windows")]
        return get_plugin_platform_info(&self.file.windows);
    }

    pub fn get_identifier(&self) -> BinaryIdentifier {
        let binary_name = BinaryName::new(self.file.owner.clone(), self.file.name.clone());
        BinaryIdentifier::new(&binary_name, &self.file.version)
    }
}

fn get_plugin_platform_info<'a>(platform_info: &'a Option<PlatformInfo>) -> Result<&'a PlatformInfo, ErrBox> {
    if let Some(platform_info) = &platform_info {
        Ok(platform_info)
    } else {
        return err!("Unsupported operating system.");
    }
}

pub async fn setup_plugin<'a, TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_file: &PluginFile,
) -> Result<BinaryManifestItem, ErrBox> {
    // download the url's bytes
    let url = plugin_file.get_url()?;
    let download_type = plugin_file.get_download_type()?;
    let url_file_bytes = environment.download_file(url).await?;
    verify_sha256_checksum(&url_file_bytes, plugin_file.get_url_checksum()?)?;

    // create folder
    let plugin_cache_dir_path = get_plugin_dir(environment, &plugin_file.get_binary_name(), &plugin_file.version());
    let _ignore = environment.remove_dir_all(&plugin_cache_dir_path);
    environment.create_dir_all(&plugin_cache_dir_path)?;

    // run the pre install command
    if let Some(pre_install_command) = plugin_file.get_pre_install_command()? {
        environment.run_shell_command(&plugin_cache_dir_path, pre_install_command)?;
    }

    // handle the setup based on the download type
    let commands = plugin_file.get_commands()?;
    verify_commands(commands)?;
    let output_dir = if let Some(output_dir) = plugin_file.get_output_dir()? {
        verify_valid_relative_path(&output_dir)?;
        let output_dir = plugin_cache_dir_path.join(output_dir);
        environment.create_dir_all(&output_dir)?;
        output_dir
    } else {
        plugin_cache_dir_path.clone()
    };
    match download_type {
        DownloadType::Zip => utils::extract_zip(
            &format!("Extracting archive for {}...", plugin_file.display(),),
            environment,
            &url_file_bytes,
            &output_dir,
        )?,
        DownloadType::TarGz => utils::extract_tar_gz(
            &format!("Extracting archive for {}...", plugin_file.display(),),
            environment,
            &url_file_bytes,
            &output_dir,
        )?,
        DownloadType::Binary => {
            if commands.len() != 1 {
                return err!("The binary download type must have exactly one command specified.");
            }
            environment.write_file(&output_dir.join(&commands[0].path), &url_file_bytes)?
        }
    }

    // run the post install command
    if let Some(post_install_command) = plugin_file.get_post_install_command()? {
        environment.run_shell_command(&plugin_cache_dir_path, post_install_command)?;
    }

    // create the shims after in case the post install fails
    environment.create_dir_all(&utils::get_shim_dir(environment))?;
    for command in commands {
        create_shim(environment, &command.name)?;
    }

    // add the plugin information to the manifest
    let item = BinaryManifestItem {
        name: plugin_file.get_binary_name(),
        version: plugin_file.version().clone(),
        created_time: environment.get_time_secs(),
        commands: commands
            .iter()
            .map(|c| BinaryManifestItemCommand {
                name: c.name.clone(),
                path: c.path.clone(),
            })
            .collect(),
        source: BinaryManifestItemSource {
            path: plugin_file.url.clone(),
            checksum: plugin_file.checksum.clone(),
        },
        environment: plugin_file.get_environment()?.clone(),
    };
    Ok(item)
}

fn verify_commands(commands: &Vec<PlatformInfoCommand>) -> Result<(), ErrBox> {
    if commands.is_empty() {
        return err!("One command must be specified.");
    }

    // prevent funny business
    for command in commands.iter() {
        verify_valid_relative_path(&command.path)?;
    }

    Ok(())
}

fn verify_valid_relative_path(path: &str) -> Result<(), ErrBox> {
    if path.contains("../") || path.contains("..\\") {
        return err!("Invalid path '{}'. A path cannot go down directories.", path);
    }
    if PathBuf::from(&path).is_absolute() {
        return err!("Invalid path '{}'. A path cannot be absolute.", path);
    }

    Ok(())
}
