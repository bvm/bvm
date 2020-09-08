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

## Setup

1. For now, install `bvm` via cargo—`cargo install bvm`.
2. Manually add the shims folder as the first item on the path:
   - Windows: `C:\Users\<user-name>\AppData\Local\bvm\bvm\shims`
   - Mac/Linux: `~/.local/share/bvm/shims`
3. Add a _.bvmrc.json_ file to your project and specify the paths to the binary manifest files.
   ```jsonc
   {
     // optional commands to run on pre and post install
     "preInstall": "",
     "postInstall": "",
     // list of binaries to use
     "binaries": [
       "https://bvm.land/deno/1.3.2.json",
       "https://bvm.land/node/14.9.0.json",
       "https://bvm.land/dprint/0.9.1.json"
     ]
   }
   ```
   Or specify with checksums to ensure the remote files don't change:
   ```jsonc
   {
     "binaries": [
       "https://bvm.land/deno/1.3.2.json@6444d03bbb4e8b0a7966f406ab0a6d190581c205291d0e082bc9a57dd8498e97",
       "https://bvm.land/dprint/0.9.1.json@52b473cd29888badf1620ea501afbd210373e6dec66d249019d1a284cf43380b"
     ]
   }
   ```
4. Run `bvm install`

## Commands

### `bvm init`

Creates an empty `.bvmrc.json` file in the current directory.

### `bvm install`

Downloads & installs the binaries in the current `.bvmrc.json` configuration file and associates them on the path with bvm if not previously done.

- Provide the `--use` flag to also use all the binaries in the configuration file on the path when outside this directory.
- Provide the `--force` flag to force an install of everything even if already installed.

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
```

### `bvm use [binary-name or owner-name/binary-name] path`

Use the version of the binary that's installed on the path if it exists.

```
# Example
bvm use deno path
```

### `bvm use`

Use all the binaries in the current configuration files globally on the path.

Generally it's not necessary to ever use this command as this happens automatically being in the current directory.

### `bvm resolve [binary name]`

Resolves the executable path of the specified binary based on the current working directory.

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

Lists the current registries

Example output:

```
denoland/deno - https://bvm.land/deno/registry.json
nodejs/node - https://bvm.land/node/registry.json
```

### `bvm install [binary-name or owner-name/binary-name] [version]`

Installs the specified binary and version based on the first matching version in the CLI's registries.

```
# Examples
bvm install --use deno 1.3.3
bvm install node 14.9.0
```

## Redirect Service

The website https://bvm.land is a redirect service. If you publish a _bvm.json_ file as a GitHub release asset (not recommended yet, due to this being a proof of concept) then you can use `https://bvm.land` to redirect to your release:

1. `https://bvm.land/<owner>/<name>/<release-tag>.json` -> `https://github.com/<owner>/<name>/releases/download/<release-tag>/bvm.json`
2. `https://bvm.land/<name>/<release-tag>.json` -> `https://github.com/<name>/<name>/releases/download/<release-tag>/bvm.json`

Example: `https://bvm.land/dprint/0.9.1.json`

## Binary manifest file

At the moment, it looks like this:

```json
{
  "schemaVersion": 1,
  "name": "deno",
  "owner": "denoland",
  "version": "1.3.1",
  "windows-x86_64": {
    "url": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-pc-windows-msvc.zip",
    "type": "zip",
    "checksum": "6ba068e517a55dd33abd60e74c38aa61ef8f45a0774578761be0107fafc3758b",
    "commands": [{
      "name": "deno",
      "path": "deno.exe"
    }],
    "preInstall": "# run any command pre installation (ex. kill process)",
    "postInstall": "# this is where you can run some commands if necessary to cause additional setup"
  },
  "linux-x86_64": {
    "url": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-unknown-linux-gnu.zip",
    "type": "zip",
    "checksum": "ef3a8740bdceab105808c91cfb918c883a23defb6719b9c511e2be30d5bfdc01",
    "commands": [{
      "name": "deno",
      "path": "deno"
    }]
  },
  "darwin-x86_64": {
    "url": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-apple-darwin.zip",
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

1. `bvm install <binary name>` - Upgrade to the latest version based on the registries.
2. Ability to list versions of a binary in the registries.
3. List 10 most similar versions found when calling `bvm install <binary-name> <version>`
4. Add warning when using `bvm install --use` and binary is in config file similar to what happens with `bvm use` (should use same code).
5. Ability to get url of version from registry.

Medium effort:

1. Ability to specify a range of supported versions in _.bvmrc.json_ to reduce the number of downloaded binaries:
   ```jsonc
   {
     "binaries": [{
       "manifest": "https://bvm.land/deno/1.3.1.json",
       "version": "^1.3.0"
     }]
   }
   ```
2. Support for file paths everywhere in addition to urls.
3. Ability to easily create and remove aliases (ex. `deno2`)
   - These should be associated with the binary they alias so when you uninstall the binary it deletes the alias.
4. Command aliases in the configuration file.
   ```jsonc
   {
     "binaries": [{
       "manifest": "https://bvm.land/deno/1.3.1.json",
       "alias": "deno-1.3.1"
     }]
   }
   ```
5. Ability to execute a specific version of an executable one time. `bvm exec deno 1.2.0 -V` or perhaps at the shim level `deno -V --bvm-use-version 1.2.0`... or maybe this should use `bvm resolve` somehow.
6. Add `bvm lock` to update the configuration file urls with checksums.
7. Multiple sub binary download locations (ex. say `npm` were installed from a different zip for `node`).

Large effort:

1. Support downstream binary dependencies (should also support a range of dependencies).

Probably unnecessary complexity:

1. `bvm use <url>` - To use a specific version of a binary globally via a url.
2. ~~Something similar to `npm run <script-name>`? Or is that out of scope?~~ Yes. I think there should be another tool people can install with bvm that does this. This tool should be very simple. There should definitely be pre and post install scripts though.
3. `bvm use <binary name> <executable file path>` for using the executable at the specified file path.
4. Ability to purge any binaries that haven't been run for X days.
5. Ability to get a specific version of a binary when using `bvm resolve` (ex. `bvm resolve deno 1.3.1`)
