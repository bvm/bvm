# bvm - Binary Version Manager

A version manager for all binaries.

![Demo using bvm command](demo.gif "Demo using bvm command")

NOTICE: This is a proof of concept. It is not recommended to use it yet as there will likely be many breaking changes.

## Goals

1. Seamless version selection based on current working directory.
2. Replace binary specific version manager tools.
3. No centralization—all urls and paths.
   - Allows for easily distributing approved binaries within an organization.
   - Easy for binary authors to distribute their applications.
4. Support completely different application binaries with the same command name.
5. Backwards compatibility loading project configuration files (once hitting 1.0)
6. **Works on Windows without needing WSL.**
7. Allows working with binaries already on the path (ex. `bvm use deno path`).

## Install

NOTICE: Don't bother trying this yet as it likely won't work.

Install by running a script based on your environment:

- Shell (Mac, Linux, WSL): `curl -fsSL https://bvm.land/install.sh | sh`
- Windows
  - [Installer](https://github.com/dsherret/bvm/releases/latest/download/bvm-x86_64-pc-windows-msvc-installer.exe)
  - Or install via powershell: `iwr https://bvm.land/install.ps1 -useb | iex`

## Global Commands

### `bvm install [url]`

Installs a binary at the specified manifest file.

```
# Examples
bvm install https://bvm.land/deno/1.3.2.json
# optionally specify a checksum
bvm install https://bvm.land/deno/1.3.2.json@6444d03bbb4e8b0a7966f406ab0a6d190581c205291d0e082bc9a57dd8498e97
# if a previous installation is on the path, use this one instead
bvm install --use https://bvm.land/deno/1.3.1.json
```

- Provide the `--use` flag to force using this binary on the path (happens automatically if nothing is on the path).
- Provide the `--force` flag to force an install even if already installed.

### `bvm use [binary-name or owner-name/binary-name] [version]`

Uses the specified binary name and version globally.

The binary and version must have been previously installed.

```
# Examples
bvm use deno 1.3.2
bvm use denoland/deno 1.3.2
bvm use name-stealer/deno 2.0.0
bvm use deno 1
bvm use deno 1.0
bvm use deno ^1.1
bvm use deno ~1.1
```

### `bvm use [binary-name or owner-name/binary-name] path`

Uses the version of the binary that's installed on the path if it exists.

```
# Example
bvm use deno path
```

### `bvm resolve [command-name]`

Resolves the executable path of the specified command name based on the current working directory.

This command is used by the created shell/batch files (shims) to tell how to resolve the file.

```
# Example
bvm resolve deno
# on windows, outputs: C:\Users\<user>\AppData\Local\bvm\bvm\binaries\denoland\deno\1.3.1\deno.exe
```

### `bvm uninstall [binary-name or owner-name/binary-name] [version]`

Uninstalls the specified binary version.

```
# Examples
bvm uninstall deno 1.2.0
bvm uninstall denoland/deno 1.3.2
bvm uninstall name-stealer/deno 2.0.0
```

### `bvm list`

Displays the installed binaries.

Example output:

```
denoland/deno 1.2.0
denoland/deno 1.3.2
dprint/dprint 9.0.1
nodejs/node 14.9.0
```

### `bvm clear-url-cache`

Clears any cached urls.

## Registry commands

Adding a registry allows you to more easily install copies of a binary without dealing with urls.

### `bvm registry add [url]`

Adds the registry at the specified url to the local CLI.

```
# Examples
bvm registry add https://bvm.land/deno/registry.json
bvm registry add https://bvm.land/node/registry.json
```

### `bvm registry remove [url]`

Removes the registry at the specified url from the local CLI.

```
# Example
bvm registry remove https://bvm.land/node/registry.json
```

### `bvm registry list`

Lists the registries saved in the CLI.

Example output:

```
denoland/deno - https://bvm.land/deno/registry.json
nodejs/node - https://bvm.land/node/registry.json
```

### `bvm install [binary-name or owner-name/binary-name]`

Installs the latest non-pre-release version of the specified binary based on the CLI's registries.

```
# Examples
bvm install deno
bvm install --use node
```

### `bvm install [binary-name or owner-name/binary-name] [version]`

Installs the specified binary and version based on the first matching version in the CLI's registries.

```
# Examples
bvm install deno 1.3.3
bvm install deno 1
bvm install deno 1.3
bvm install deno ^1.3
bvm install --use node 14.9.0
```

## Projects

`bvm` allows for specifying versions of binaries to automatically use within a current directory.

### Setup

1. Run `bvm init` in the project's root directory.
2. Open up the created _.bvmrc.json_ file and specify the paths to the binary manifest files.
   ```jsonc
   {
     // optional commands to run on pre and post install
     "onPreInstall": "",
     "onPostInstall": "",
     // list of binaries to use
     "binaries": [
       // Either specify:
       // 1. Urls
       "https://bvm.land/node/14.9.0.json",
       // 2. Urls with a checksum to ensure the remote file doesn't change
       "https://bvm.land/dprint/0.9.1.json@52b473cd29888badf1620ea501afbd210373e6dec66d249019d1a284cf43380b",
       // 3. Objects
       {
         "path": "https://bvm.land/deno/1.3.2.json",
         "checksum": "6444d03bbb4e8b0a7966f406ab0a6d190581c205291d0e082bc9a57dd8498e97", // optional for path above
         "version": "^1.3.0" // optional, won't install specified url if user has a version installed that matches
       }
     ]
   }
   ```
3. Run `bvm install`

### Commands

### `bvm init`

Creates an empty `.bvmrc.json` file in the current directory.

### `bvm install`

Downloads & installs the binaries in the current `.bvmrc.json` configuration file and associates them on the path with bvm if not previously done.

- Provide the `--use` flag to also use all the binaries in the configuration file on the path when outside this directory.
- Provide the `--force` flag to force an install of everything even if already installed or has a matching version.

### `bvm add [url]`

Adds the specified binary at the specified url to a project's `.bvmrc.json` file based on the current directory. Installs if necessary.

```
# Example
bvm add https://bvm.land/deno/1.3.2.json
```

Configuration file would then contain:

```jsonc
{
  "binaries": [
    {
      "path": "https://bvm.land/deno/1.3.2.json",
      "checksum": "6444d03bbb4e8b0a7966f406ab0a6d190581c205291d0e082bc9a57dd8498e97",
      "version": "^1.3.2"
    }
  ]
}
```

### `bvm add [binary-name or owner-name/binary-name] [version]`

Adds the specified binary from a registry to a project's `.bvmrc.json` file based on the current directory. Installs if necessary.

The version is optional.

```
# Examples
bvm add deno 1.3.1
bvm add name-stealer/deno 2.0.0
bvm add node
```

### `bvm use`

Uses all the binaries in the current configuration files globally on the path.

Generally it's not necessary to ever use this command as this happens automatically being in the current directory.

## Redirect Service

The website https://bvm.land is a redirect service. If you publish a _bvm.json_ file as a GitHub release asset (not recommended yet, due to this being a proof of concept) then you can use `https://bvm.land` to redirect to your release:

1. `https://bvm.land/<owner>/<name>/<release-tag>.json` -> `https://github.com/<owner>/<name>/releases/download/<release-tag>/bvm.json`
2. `https://bvm.land/<name>/<release-tag>.json` -> `https://github.com/<name>/<name>/releases/download/<release-tag>/bvm.json`

Example: `https://bvm.land/dprint/0.9.1.json`

## Binary manifest file

At the moment, it looks like this:

```jsonc
{
  "schemaVersion": 1,
  "name": "deno",
  "owner": "denoland",
  "version": "1.3.1",
  "description": "A secure JavaScript and TypeScript runtime.",
  "windows-x86_64": {
    "path": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-pc-windows-msvc.zip",
    "type": "zip",
    "checksum": "6ba068e517a55dd33abd60e74c38aa61ef8f45a0774578761be0107fafc3758b",
    "commands": [{
      "name": "deno",
      "path": "deno.exe"
    }],
    "onPreInstall": "# run any command pre installation (ex. kill process)",
    "onPostInstall": "# this is where you can run some commands if necessary to cause additional setup",
    "onUse": "# command to execute when using this",
    "onStopUse": "# command to execute when stopping use of this"
  },
  "linux-x86_64": {
    "path": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-unknown-linux-gnu.zip",
    "type": "zip",
    "checksum": "ef3a8740bdceab105808c91cfb918c883a23defb6719b9c511e2be30d5bfdc01",
    "commands": [{
      "name": "deno",
      "path": "deno"
    }]
  },
  "darwin-x86_64": {
    "path": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-apple-darwin.zip",
    "type": "zip",
    "checksum": "b1bc5de79b71c3f33d0151486249d088f5f5604126812dc55b1dd21b28704d8a",
    "commands": [{
      "name": "deno",
      "path": "deno"
    }]
  }
}
```

Supported types: `zip`, `exe`, `tar.gz` (will add more later)

Other examples:

- Multiple commands: [https://bvm.land/node/14.9.0.json](https://bvm.land/node/14.9.0.json)

## Future improvements

Low effort:

1. Ability to list versions of a binary in the registries.
2. List 10 most similar versions found when calling `bvm install <binary-name> <version>` and there are no matches.
3. Ability to get url of version from registry.
4. Perhaps rename "registry" to something else since it's a binary per registry.
5. Document why there won't be support for multiple binaries per registry (open an issue and write it in there).
6. Output the binary owner/name, description, and recent versions after adding a registry.
7. Add command to ensure all binaries in the manifest file are installed (when using Windows, this is useful for when a user goes on a different computer since the binaries are stored locally). It should also "use" any binaries as specified in the configuration.
8. Add "onPostUninstall" script
9. Add an `--isolate` flag for `bvm exec`.
10. Validate group and binary names. Probably use the same rules https://www.npmjs.com/package/validate-npm-package-name#naming-rules and add no `bvm` binary name allowed.

Medium effort:

1. Ability to easily create and remove aliases (ex. `deno2`)
   - These should be associated with the binary they alias so when you uninstall the binary it deletes the alias.
2. Command aliases in the configuration file.
   ```jsonc
   {
     "binaries": [{
       "path": "https://bvm.land/deno/1.3.1.json",
       "alias": "deno-1.3.1"
     }]
   }
   ```
3. Ability to execute a specific version of an executable one time. (ex. `bvm exec deno 1.2.0 deno -V` -- will probably require command name unfortunately).
4. Add `bvm lock` to update the configuration file urls with checksums.
5. Multiple sub binary download locations (ex. say `npm` were installed from a different zip for `node`).
6. Add key/value setting storage. Example: `bvm setting nodejs/node <key> <value>` and `bvm setting nodejs/node <key>` to get. Use this instead of environment variables.

Large effort:

1. Support downstream binary dependencies (should also support a range of dependencies).

Future:

1. Provide a setting in the app for changing the local data directory (where binaries are installed). This should not be an environment variable because it should also update all the system paths appropriately and move everything over.
2. Allow specifying environment variables per directory. This seems to have been done in [direnv](https://github.com/direnv/direnv) so it should be possible here. See also, https://unix.stackexchange.com/a/21364/128067 and https://unix.stackexchange.com/a/170282/128067 (seems like it's better not to override the command and instead use shell hooks--ex. `PROMPT_COMMAND` and tell if the directory changed). For powershell, https://github.com/takekazuomi/posh-direnv. I'm not sure about cmd and would be surprised

Far future:

1. Support breaking up registries into multiple files (ex. it would give a semver range for a file internally). This would only be useful for extremely large files.
2. Probably editor extensions to properly set the environment variables based on the open project. Ex. you change the folder in VSCode and it properly updates its process' environment variables... wouldn't be hard to do.

Probably unnecessary complexity:

1. `bvm use <url>` - To use a specific version of a binary globally via a url.
2. ~~Something similar to `npm run <script-name>`? Or is that out of scope?~~ Yes. I think there should be another tool people can install with bvm that does this. This tool should be very simple. There should definitely be pre and post install scripts though.
3. `bvm use <binary name> <executable file path>` for using the executable at the specified file path.
4. Ability to purge any binaries that haven't been run for X days.
5. Ability to get a specific version of a binary when using `bvm resolve` (ex. `bvm resolve deno 1.3.1`)
6. Consider creating a `bvm resolve-v1` hidden sub command. Too much complexity. Better to just have a command to recreate the shims.
7. Support for file paths everywhere in addition to urls.
