use dprint_cli_core::checksums::ChecksumPathOrUrl;
use dprint_cli_core::types::ErrBox;

use super::manifest::get_manifest_file_path;
use super::setup::{get_plugin_file, get_shim_path, setup_plugin, PluginFile};
use super::{get_plugin_dir, helpers};
use super::{BinaryIdentifier, BinaryManifestItem, GlobalBinaryLocation, PluginsManifest};
use crate::configuration::ConfigFileBinary;
use crate::environment::Environment;
use crate::types::{BinaryName, CommandName, VersionSelector};
use crate::utils;

pub enum UrlInstallAction {
    None,
    Install(PluginFile),
}

/// Used to make changes to the plugins manifest.
pub struct PluginsMut<TEnvironment: Environment> {
    environment: TEnvironment,
    pub manifest: PluginsManifest,
}

impl<TEnvironment: Environment> PluginsMut<TEnvironment> {
    pub fn new(environment: TEnvironment, manifest: PluginsManifest) -> Self {
        PluginsMut { environment, manifest }
    }

    pub fn load(environment: &TEnvironment) -> Result<Self, ErrBox> {
        let manifest = PluginsManifest::load(environment)?;
        Ok(PluginsMut::new(environment.clone(), manifest))
    }

    // general

    pub async fn setup_plugin<'a>(&'a mut self, plugin_file: &PluginFile) -> Result<&'a BinaryManifestItem, ErrBox> {
        let item = setup_plugin(&self.environment, plugin_file).await?;
        let identifier = item.get_identifier();
        self.manifest.binaries.insert(identifier.clone(), item);
        Ok(self.manifest.get_binary(&identifier).unwrap())
    }

    pub async fn get_url_install_action(
        &mut self,
        checksum_url: &ChecksumPathOrUrl,
        version_selector: Option<&VersionSelector>,
        force_install: bool,
    ) -> Result<UrlInstallAction, ErrBox> {
        // always install the url version for force
        if force_install {
            return Ok(UrlInstallAction::Install(
                self.get_and_associate_plugin_file(checksum_url).await?,
            ));
        }

        // check the cache for if the identifier is saved
        let identifier = self.manifest.get_identifier_from_url(&checksum_url);

        // use the exact version if installed
        if let Some(identifier) = &identifier {
            if self.manifest.has_binary(identifier) {
                self.error_if_identifier_not_matches_version_selector(&identifier, &version_selector)?;
                return Ok(UrlInstallAction::None);
            }
        }

        let plugin_file = self.get_and_associate_plugin_file(checksum_url).await?;
        let identifier = plugin_file.get_identifier();

        self.error_if_identifier_not_matches_version_selector(&identifier, &version_selector)?;

        // check again if it's installed after associating the plugin file to an identifier
        if self.manifest.has_binary(&identifier) {
            return Ok(UrlInstallAction::None);
        }

        // check if a version is installed that matches the provided version, if so use that
        if let Some(version_selector) = &version_selector {
            let name_selector = identifier.get_binary_name().to_selector();
            let binary =
                helpers::get_latest_binary_matching_name_and_version(&self.manifest, &name_selector, version_selector);
            if binary.is_some() {
                return Ok(UrlInstallAction::None);
            }
        }

        // install the specified url's plugin file
        Ok(UrlInstallAction::Install(plugin_file))
    }

    fn error_if_identifier_not_matches_version_selector(
        &self,
        identifier: &BinaryIdentifier,
        version_selector: &Option<&VersionSelector>,
    ) -> Result<(), ErrBox> {
        if let Some(version_selector) = version_selector {
            let version = identifier.get_version();
            if !version_selector.matches(&version) {
                return err!("The specified version '{}' did not match '{}' in the path file. Please specify a different path or update the version.", version_selector, version);
            }
        }
        Ok(())
    }

    async fn get_and_associate_plugin_file(&mut self, checksum_url: &ChecksumPathOrUrl) -> Result<PluginFile, ErrBox> {
        let plugin_file = get_plugin_file(&self.environment, checksum_url).await?;
        // associate the url to the binary identifier
        let identifier = plugin_file.get_identifier();
        self.set_identifier_for_url(&checksum_url, identifier);
        self.save()?; // todo: remove?
        Ok(plugin_file)
    }

    pub fn set_global_binary_if_not_set(
        &mut self,
        identifier: &BinaryIdentifier,
        command_name: &CommandName,
    ) -> Result<bool, ErrBox> {
        Ok(if self.manifest.get_global_binary_location(&command_name).is_none() {
            if utils::get_path_executable_path(&self.environment, &command_name)?.is_some() {
                self.use_global_version(command_name.clone(), GlobalBinaryLocation::Path)?;
                false
            } else {
                self.use_global_version(command_name.clone(), GlobalBinaryLocation::Bvm(identifier.clone()))?;
                true
            }
        } else {
            self.manifest.is_global_version(identifier, command_name)
        })
    }

    pub async fn get_installed_binary_for_config_binary(
        &mut self,
        config_binary: &ConfigFileBinary,
    ) -> Result<Option<&BinaryManifestItem>, ErrBox> {
        // associate the url to an identifier in order to be able to tell the name
        if self.manifest.get_identifier_from_url(&config_binary.path).is_none() {
            self.get_and_associate_plugin_file(&config_binary.path).await?;
        }

        // now get the binary item based on the config file
        Ok(helpers::get_installed_binary_if_associated_config_file_binary(
            &self.manifest,
            &config_binary,
        ))
    }

    // pending environment changes

    pub fn set_identifier_for_url(&mut self, url: &ChecksumPathOrUrl, identifier: BinaryIdentifier) {
        self.manifest
            .urls_to_identifier
            .insert(url.path_or_url.clone(), identifier);
    }

    pub fn clear_cached_urls(&mut self) {
        self.manifest.urls_to_identifier.clear();
    }

    pub fn clear_pending_env_changes(&mut self) {
        self.manifest.pending_env_changes.clear();
    }

    // binary environment paths

    fn add_bin_env_paths(&mut self, paths: Vec<String>) {
        for path in paths {
            if !self.manifest.binary_paths.contains(&path) {
                self.manifest.binary_paths.push(path);
            }
        }
    }

    fn remove_bin_env_paths(&mut self, paths: &Vec<String>) {
        for path in paths.iter() {
            if let Some(pos) = self.manifest.binary_paths.iter().position(|p| p == path) {
                self.manifest.binary_paths.remove(pos);
            }
        }
    }

    // binaries

    pub fn remove_binary(&mut self, identifier: &BinaryIdentifier) -> Result<(), ErrBox> {
        let previous_global_command_names = self.manifest.get_global_command_names(&identifier);
        let binary_info = if let Some(item) = self.manifest.get_binary(identifier) {
            Some((item.name.clone(), item.get_command_names()))
        } else {
            None
        };

        self.manifest.binaries.remove(identifier);

        if let Some((binary_name, command_names)) = binary_info {
            // update the selected global binary
            for command_name in command_names {
                if !self.manifest.has_binary_with_command(&command_name) {
                    self.remove_global_binary(&command_name)?; // could be removing the path entry
                } else {
                    self.remove_if_global_binary(&binary_name, &command_name, identifier)?;
                }
            }
        }

        // check if this is the last binary with this command. If so, delete the shim
        for command_name in previous_global_command_names.iter() {
            if !self.manifest.has_binary_with_command(&command_name) {
                self.environment
                    .remove_file(&get_shim_path(&self.environment, &command_name)?)?;
            }
        }
        Ok(())
    }

    pub fn use_global_version(
        &mut self,
        command_name: CommandName,
        location: GlobalBinaryLocation,
    ) -> Result<(), ErrBox> {
        let new_identifier = location.to_identifier_option();

        if let Some(new_identifier) = &new_identifier {
            if !self.manifest.has_any_global_command(&new_identifier) {
                if let Some(binary) = self.manifest.get_binary(&new_identifier) {
                    if let Some(on_use_command) = &binary.on_use {
                        let plugin_dir = get_plugin_dir(&self.environment, &binary.name, &binary.version)?;
                        self.environment.run_shell_command(&plugin_dir, &on_use_command)?;
                    }
                }

                if self.manifest.has_environment_paths(&new_identifier) {
                    self.manifest
                        .pending_env_changes
                        .mark_for_adding(new_identifier.clone());
                }
            }
        }

        self.remove_global_binary(&command_name)?;
        self.manifest.global_versions.set(command_name, location);
        Ok(())
    }

    fn remove_if_global_binary(
        &mut self,
        removed_binary_name: &BinaryName,
        removed_command_name: &CommandName,
        removed_binary_identifier: &BinaryIdentifier,
    ) -> Result<(), ErrBox> {
        if let Some(GlobalBinaryLocation::Bvm(current_identifier)) =
            self.manifest.global_versions.get(removed_command_name)
        {
            if &current_identifier == removed_binary_identifier {
                // set the latest binary as the global binary
                let latest_binary = self
                    .manifest
                    .get_latest_binary_with_name(&removed_binary_name)
                    .or_else(|| self.manifest.get_latest_binary_with_command(removed_command_name));
                if let Some(latest_binary) = latest_binary {
                    let latest_identifier = latest_binary.get_identifier();
                    self.use_global_version(removed_command_name.clone(), latest_identifier.into())?;
                } else {
                    self.remove_global_binary(removed_command_name)?;
                }
            }
        }
        Ok(())
    }

    fn remove_global_binary(&mut self, command_name: &CommandName) -> Result<(), ErrBox> {
        let past_location = self.manifest.get_global_binary_location(&command_name);
        let past_identifier = past_location.map(|l| l.to_identifier_option()).flatten();
        self.manifest.global_versions.remove(command_name);

        if let Some(past_identifier) = past_identifier {
            if !self.manifest.has_any_global_command(&past_identifier) {
                if self.manifest.has_environment_paths(&past_identifier) {
                    self.manifest
                        .pending_env_changes
                        .mark_for_removal(past_identifier.clone());
                }

                if let Some(binary) = self.manifest.get_binary(&past_identifier) {
                    if let Some(on_stop_use_command) = &binary.on_stop_use {
                        let plugin_dir = get_plugin_dir(&self.environment, &binary.name, &binary.version)?;
                        self.environment.run_shell_command(&plugin_dir, &on_stop_use_command)?;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn save(&mut self) -> Result<(), ErrBox> {
        // handle any pending changes
        if self.manifest.pending_env_changes.any() {
            // update the environment variables on windows (the environment manifest will be be set on the path on linux shell startup)
            #[cfg(target_os = "windows")]
            {
                let local_data_dir = self.environment.get_local_user_data_dir()?;
                for path in self.manifest.get_relative_pending_added_paths() {
                    self.environment
                        .ensure_system_path(&local_data_dir.join(path).to_string_lossy())?;
                }
                for path in self.manifest.get_relative_pending_removed_paths() {
                    self.environment
                        .remove_system_path(&local_data_dir.join(path).to_string_lossy())?;
                }
            }

            // update binary environment paths
            self.add_bin_env_paths(self.manifest.get_relative_pending_added_paths());
            self.remove_bin_env_paths(&self.manifest.get_relative_pending_removed_paths());
        }

        // save plugin file
        let file_path = get_manifest_file_path(&self.environment)?;
        let serialized_manifest = serde_json::to_string(&self.manifest)?;
        self.environment.write_file_text(&file_path, &serialized_manifest)?;

        Ok(())
    }
}
