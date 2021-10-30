use dprint_cli_core::types::ErrBox;

use super::helpers;
use super::manifest::get_manifest_file_path;
use super::setup::get_plugin_file;
use super::setup::get_shim_paths;
use super::setup::setup_plugin;
use super::setup::PluginFile;
use super::BinaryIdentifier;
use super::BinaryManifestItem;
use super::GlobalBinaryLocation;
use super::PluginsManifest;
use crate::configuration::ConfigFileBinary;
use crate::environment::Environment;
use crate::types::BinaryName;
use crate::types::CommandName;
use crate::types::VersionSelector;
use crate::utils;
use crate::utils::ChecksumUrl;

pub enum UrlInstallAction {
    None,
    Install(PluginFile),
}

/// Used to make changes to the plugins manifest.
pub struct PluginsMut<TEnvironment: Environment> {
    environment: TEnvironment,
    pub manifest: PluginsManifest,
    allow_write: bool,
}

impl<TEnvironment: Environment> PluginsMut<TEnvironment> {
    fn new(environment: TEnvironment, allow_write: bool) -> Self {
        let manifest = PluginsManifest::load(&environment);
        PluginsMut {
            environment,
            manifest,
            allow_write,
        }
    }

    pub fn load(environment: &TEnvironment) -> Self {
        PluginsMut::new(environment.clone(), true)
    }

    pub fn load_disallow_write(environment: &TEnvironment) -> Self {
        PluginsMut::new(environment.clone(), false)
    }

    pub fn from_manifest_disallow_write(environment: &TEnvironment, manifest: PluginsManifest) -> Self {
        PluginsMut {
            environment: environment.clone(),
            manifest,
            allow_write: false,
        }
    }

    // general

    pub fn setup_plugin<'a>(&'a mut self, plugin_file: &PluginFile) -> Result<&'a BinaryManifestItem, ErrBox> {
        let item = setup_plugin(&self.environment, plugin_file)?;
        let identifier = item.get_identifier();
        self.manifest.binaries.insert(identifier.clone(), item);
        Ok(self.manifest.get_binary(&identifier).unwrap())
    }

    pub fn get_url_install_action(
        &mut self,
        checksum_url: &ChecksumUrl,
        version_selector: Option<&VersionSelector>,
        force_install: bool,
    ) -> Result<UrlInstallAction, ErrBox> {
        // always install the url version for force
        if force_install {
            return Ok(UrlInstallAction::Install(
                self.get_and_associate_plugin_file(checksum_url)?,
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

        let plugin_file = self.get_and_associate_plugin_file(checksum_url)?;
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

    fn get_and_associate_plugin_file(&mut self, checksum_url: &ChecksumUrl) -> Result<PluginFile, ErrBox> {
        let plugin_file = get_plugin_file(&self.environment, checksum_url)?;
        // associate the url to the binary identifier
        let identifier = plugin_file.get_identifier();
        self.set_identifier_for_url(checksum_url, identifier);
        self.save()?; // todo: remove?
        Ok(plugin_file)
    }

    pub fn set_global_binary_if_not_set(
        &mut self,
        identifier: &BinaryIdentifier,
        command_name: &CommandName,
    ) -> Result<bool, ErrBox> {
        Ok(if self.manifest.get_global_binary_location(&command_name).is_none() {
            if utils::get_path_executable_path(&self.environment, &command_name).is_some() {
                self.use_global_version(command_name, GlobalBinaryLocation::Path)?;
                false
            } else {
                self.use_global_version(command_name, GlobalBinaryLocation::Bvm(identifier.clone()))?;
                true
            }
        } else {
            self.manifest.is_global_version(identifier, command_name)
        })
    }

    pub fn get_installed_binary_for_config_binary(
        &mut self,
        config_binary: &ConfigFileBinary,
    ) -> Result<Option<&BinaryManifestItem>, ErrBox> {
        // associate the url to an identifier in order to be able to tell the name
        self.ensure_url_associated(&config_binary.url)?;

        // now get the binary item based on the config file
        Ok(helpers::get_installed_binary_if_associated_config_file_binary(
            &self.manifest,
            &config_binary,
        ))
    }

    pub fn ensure_url_associated(&mut self, url: &ChecksumUrl) -> Result<(), ErrBox> {
        // associate the url to an identifier in order to be able to tell the name
        if self.manifest.get_identifier_from_url(&url).is_none() {
            self.get_and_associate_plugin_file(&url)?;
        }
        Ok(())
    }

    // pending environment changes

    pub fn set_identifier_for_url(&mut self, url: &ChecksumUrl, identifier: BinaryIdentifier) {
        self.manifest.urls_to_identifier.insert(url.url.to_string(), identifier);
    }

    pub fn clear_cached_urls(&mut self) {
        self.manifest.urls_to_identifier.clear();
    }

    pub fn clear_pending_env_changes(&mut self) {
        self.manifest.pending_env_changes.clear();
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
                for shim_path in get_shim_paths(&self.environment, &command_name) {
                    self.environment.remove_file(&shim_path)?;
                }
            }
        }
        Ok(())
    }

    pub fn use_global_version(
        &mut self,
        command_name: &CommandName,
        location: GlobalBinaryLocation,
    ) -> Result<(), ErrBox> {
        self.remove_global_binary(command_name)?;

        let new_identifier = location.to_identifier_option();
        if let Some(new_identifier) = &new_identifier {
            if !self.manifest.has_any_global_command(&new_identifier) {
                if self.manifest.has_environment_changes(&new_identifier) {
                    self.manifest
                        .pending_env_changes
                        .mark_for_adding(new_identifier.clone());
                }
            }
        }

        self.manifest.global_versions.set(command_name.clone(), location);

        // recreate the shim with the latest version
        helpers::recreate_shim(&self.environment, &self.manifest, command_name)?;

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
                    self.use_global_version(removed_command_name, latest_identifier.into())?;
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
                if self.manifest.has_environment_changes(&past_identifier) {
                    self.manifest
                        .pending_env_changes
                        .mark_for_removal(past_identifier.clone());
                }
            }
        }

        Ok(())
    }

    pub fn save(&mut self) -> Result<(), ErrBox> {
        if !self.allow_write {
            panic!("Internal error: Cannot save when allow_write is false.");
        }

        // handle any pending changes
        if self.manifest.pending_env_changes.any() {
            // update the environment variables on windows (the environment manifest will be be set on the path on linux shell startup)
            #[cfg(target_os = "windows")]
            {
                for path in self.manifest.get_relative_pending_removed_paths(&self.environment) {
                    self.environment.remove_system_path(&path)?;
                }
                for path in self.manifest.get_relative_pending_added_paths(&self.environment) {
                    self.environment.ensure_system_path(&path)?;
                }

                for (key, _) in self.manifest.get_pending_removed_env_variables(&self.environment) {
                    self.environment.remove_env_variable(&key)?;
                }

                for (key, value) in self.manifest.get_pending_added_env_variables(&self.environment) {
                    self.environment.set_env_variable(key, value)?;
                }
            }
        }

        // save plugin file
        let file_path = get_manifest_file_path(&self.environment);
        let serialized_manifest = serde_json::to_string(&self.manifest)?;
        self.environment.write_file_text(&file_path, &serialized_manifest)?;

        Ok(())
    }
}
