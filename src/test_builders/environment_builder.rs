use super::{BvmrcBuilder, PluginFileBuilder};
use crate::environment::{Environment, TestEnvironment};

use std::io::Write;
use std::path::PathBuf;

pub struct EnvironmentBuilder {
    environment: TestEnvironment,
}

impl EnvironmentBuilder {
    pub fn new() -> EnvironmentBuilder {
        EnvironmentBuilder {
            environment: TestEnvironment::new(),
        }
    }

    pub fn build(&self) -> TestEnvironment {
        self.environment.clone()
    }

    pub fn create_remote_zip_package(&self, url: &str, owner: &str, name: &str, version: &str) -> String {
        let mut builder = self.create_plugin_builder(url, owner, name, version);
        builder.download_type(PluginDownloadType::Zip);
        builder.build()
    }

    pub fn create_remote_zip_multiple_commands_package(
        &self,
        url: &str,
        owner: &str,
        name: &str,
        version: &str,
    ) -> String {
        let mut builder = self.create_plugin_builder(url, owner, name, version);
        builder.add_command(&format!("{}-second", name));
        builder.download_type(PluginDownloadType::Zip);
        builder.build()
    }

    pub fn create_remote_tar_gz_package(&self, url: &str, owner: &str, name: &str, version: &str) -> String {
        let mut builder = self.create_plugin_builder(url, owner, name, version);
        builder.download_type(PluginDownloadType::TarGz);
        builder.build()
    }

    pub fn create_plugin_builder(&self, url: &str, owner: &str, name: &str, version: &str) -> PluginBuilder {
        let mut builder = PluginBuilder::new(self.environment.clone());

        builder
            .url(url)
            .owner(owner)
            .name(name)
            .version(version)
            .description("Some description")
            .add_command(name);

        builder
    }

    pub fn add_binary_to_path(&self, name: &str) -> String {
        let path_dir = PathBuf::from("/path-dir");
        if !self.environment.get_system_path_dirs().contains(&path_dir) {
            self.environment.add_path_dir(path_dir);
        }
        let path_exe_path = if cfg!(target_os = "windows") {
            format!("/path-dir\\{}.bat", name)
        } else {
            format!("/path-dir/{}", name)
        };
        self.environment
            .write_file_text(&PathBuf::from(&path_exe_path), "")
            .unwrap();
        path_exe_path
    }

    pub fn create_bvmrc(&self, binaries: Vec<&str>) {
        let mut builder = self.create_bvmrc_builder();
        for url in binaries.into_iter() {
            builder.add_binary_path(url);
        }
        builder.build();
    }

    pub fn create_bvmrc_builder(&self) -> BvmrcBuilder {
        BvmrcBuilder::new(&self.environment)
    }

    pub fn create_remote_registry_file(
        &self,
        url: &str,
        owner: &str,
        name: &str,
        items: Vec<crate::registry::RegistryVersionInfo>,
    ) {
        let file_text = format!(
            r#"{{
    "schemaVersion": 1,
    "binaries": [{{
        "owner": "{}",
        "name": "{}",
        "description": "Some description.",
        "versions": [{}]
    }}]
}}"#,
            owner,
            name,
            items
                .into_iter()
                .map(|item| format!(
                    r#"{{"version": "{}", "path": "{}", "checksum": "{}"}}"#,
                    item.version, item.path, item.checksum
                ))
                .collect::<Vec<_>>()
                .join(",")
        );
        self.environment.add_remote_file(url, file_text.into_bytes());
    }
}

pub enum PluginDownloadType {
    Zip,
    TarGz,
}

pub struct PluginBuilder {
    environment: TestEnvironment,
    file: PluginFileBuilder,
    download_type: Option<PluginDownloadType>,
    url: Option<String>,
}

impl PluginBuilder {
    pub fn new(environment: TestEnvironment) -> Self {
        PluginBuilder {
            environment,
            file: PluginFileBuilder::new(),
            download_type: None,
            url: None,
        }
    }

    pub fn build(&mut self) -> String {
        assert_eq!(
            self.download_type.is_some(),
            true,
            "set a download type before building"
        );
        let file_text = self.file.to_json_text();
        let checksum = dprint_cli_core::checksums::get_sha256_checksum(file_text.as_bytes());
        self.environment.add_remote_file(
            self.url.as_ref().expect("Need to set a url before building."),
            file_text.into_bytes(),
        );
        checksum
    }

    pub fn url<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.url = Some(value.to_string());
        self
    }

    pub fn name<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.name(value);
        self
    }

    pub fn owner<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.owner(value);
        self
    }

    pub fn version<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.version(value);
        self
    }

    pub fn description<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.description(value);
        self
    }

    pub fn add_env_path<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.windows().add_env_path(value);
        self.file.linux().add_env_path(value);
        self.file.mac().add_env_path(value);
        self
    }

    pub fn on_pre_install<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.windows().on_pre_install(value);
        self.file.linux().on_pre_install(value);
        self.file.mac().on_pre_install(value);
        self
    }

    pub fn on_post_install<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.windows().on_post_install(value);
        self.file.linux().on_post_install(value);
        self.file.mac().on_post_install(value);
        self
    }

    pub fn on_use<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.windows().on_use(value);
        self.file.linux().on_use(value);
        self.file.mac().on_use(value);
        self
    }

    pub fn on_stop_use<'a>(&'a mut self, value: &str) -> &'a mut PluginBuilder {
        self.file.windows().on_stop_use(value);
        self.file.linux().on_stop_use(value);
        self.file.mac().on_stop_use(value);
        self
    }

    pub fn add_command<'a>(&'a mut self, name: &str) -> &'a mut PluginBuilder {
        assert_eq!(
            self.download_type.is_some(),
            false,
            "cannot add a command after setting the download type"
        );
        self.file.windows().add_command(&name, &format!("{}.exe", name));
        self.file.linux().add_command(&name, &name);
        self.file.mac().add_command(&name, &name);
        self
    }

    pub fn download_type<'a>(&'a mut self, download_type: PluginDownloadType) -> &'a mut PluginBuilder {
        match &download_type {
            PluginDownloadType::TarGz => self.setup_binaries_tar_gz(),
            PluginDownloadType::Zip => self.setup_binaries_zip(),
        }
        self.download_type = Some(download_type);
        self
    }

    fn setup_binaries_tar_gz(&mut self) {
        let commands = self.file.get_command_names();
        let name = self.file.get_name();
        let version = self.file.get_version().as_str().to_string();
        assert_eq!(
            commands.is_empty(),
            false,
            "you should set add at least one command before download type"
        );
        assert_eq!(name.is_empty(), false, "set a name before download type");
        assert_eq!(version.is_empty(), false, "set a version before download type");

        let windows_tar_gz_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-windows.tar.gz",
            version, name
        );
        let windows_checksum = create_remote_tar_gz(&self.environment, &windows_tar_gz_url, true, &commands);
        let mac_tar_gz_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-mac.tar.gz",
            version, name
        );
        let mac_checksum = create_remote_tar_gz(&self.environment, &mac_tar_gz_url, false, &commands);
        let linux_tar_gz_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-linux.tar.gz",
            version, name
        );
        let linux_checksum = create_remote_tar_gz(&self.environment, &linux_tar_gz_url, false, &commands);

        self.file
            .windows()
            .path(&windows_tar_gz_url)
            .checksum(&windows_checksum)
            .download_type("tar.gz");
        self.file
            .linux()
            .path(&linux_tar_gz_url)
            .checksum(&linux_checksum)
            .download_type("tar.gz");
        self.file
            .mac()
            .path(&mac_tar_gz_url)
            .checksum(&mac_checksum)
            .download_type("tar.gz");
    }

    fn setup_binaries_zip(&mut self) {
        let commands = self.file.get_command_names();
        assert_eq!(commands.is_empty(), false, "you should set add at least one command");
        let name = self.file.get_name();
        let version = self.file.get_version().as_str().to_string();
        let windows_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-windows.zip",
            version, name
        );
        let windows_checksum = create_remote_zip(&self.environment, &windows_zip_url, true, &commands);
        let mac_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-mac.zip",
            version, name
        );
        let mac_checksum = create_remote_zip(&self.environment, &mac_zip_url, false, &commands);
        let linux_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-linux.zip",
            version, name
        );
        let linux_checksum = create_remote_zip(&self.environment, &linux_zip_url, false, &commands);

        self.file.windows().path(&windows_zip_url).checksum(&windows_checksum);
        self.file.linux().path(&linux_zip_url).checksum(&linux_checksum);
        self.file.mac().path(&mac_zip_url).checksum(&mac_checksum);
    }
}

fn create_remote_zip(environment: &TestEnvironment, url: &str, is_windows: bool, commands: &Vec<String>) -> String {
    let buf: Vec<u8> = Vec::new();
    let w = std::io::Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(w);
    let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for command in commands.iter() {
        let file_name = if is_windows {
            format!("{}.exe", command)
        } else {
            command.to_string()
        };
        zip.start_file(&file_name, options).unwrap();
        zip.write(format!("test-{}-{}", command, url).as_bytes()).unwrap();
    }
    let result = zip.finish().unwrap().into_inner();
    let zip_file_checksum = dprint_cli_core::checksums::get_sha256_checksum(&result);
    environment.add_remote_file(url, result);
    zip_file_checksum
}

fn create_remote_tar_gz(environment: &TestEnvironment, url: &str, is_windows: bool, commands: &Vec<String>) -> String {
    use flate2::write::GzEncoder;
    use flate2::Compression;

    let buf: Vec<u8> = Vec::new();
    let w = std::io::Cursor::new(buf);
    let mut archive = tar::Builder::new(w);

    for command in commands.iter() {
        let file_name = if is_windows {
            format!("{}.exe", command)
        } else {
            command.to_string()
        };
        let data = format!("test-{}-{}", command, url);

        let mut header = tar::Header::new_gnu();
        header.set_path(file_name).unwrap();
        header.set_size(data.len() as u64);
        header.set_cksum();
        archive.append(&header, data.as_bytes()).unwrap();
    }

    archive.finish().unwrap();

    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(&archive.into_inner().unwrap().into_inner()).unwrap();
    let result = e.finish().unwrap();

    let tar_gz_file_checksum = dprint_cli_core::checksums::get_sha256_checksum(&result);
    environment.add_remote_file(url, result);
    tar_gz_file_checksum
}
