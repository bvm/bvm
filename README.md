# NOT MAINTAINED - bvm - Binary Version Manager

A version manager for all binaries.

NOTICE: This is no longer maintained. Although I think it's a good idea, I ran into too much trouble with tools making too many assumptions and not handling things properly for this to be workable. Additionally, there are some major limitations with batch scripting on Windows that I couldn't figure out that leads to some edge cases in cmd. I would highly recommend to NOT USE THIS.

## Goals

1. Replace binary specific version manager tools.
2. Provide an easy way for binary authors to distribute and have their users manage versions.
3. Cross platform - Provide a good experience in as many shells as possible (**Does not require WSL on Windows**).
4. Backwards compatibility (once hitting 1.0).
5. Distributed—use urls and paths.
   - Allows for easily distributing approved binaries within an organization.
   - Easy for binary authors to distribute their applications as there is no approval delay.
6. Seamless version selection based on current working directory.
7. Allow working with binaries already on the path.
8. Support completely different application binaries with the same command name.

## Install

Install by running a script based on your environment:

- Shell (Mac, Linux, WSL): `curl -fsSL https://bvm.land/install.sh | sh`
- Cmd, Powershell (Windows):
  - [Installer](https://github.com/dsherret/bvm/releases/latest/download/bvm-x86_64-pc-windows-msvc-installer.exe)
  - Or via powershell: `iwr https://bvm.land/install.ps1 -useb | iex`

## CI

- [GitHub action](https://github.com/bvm/gh-action)
- More to come...

## Global Commands

### `bvm install <url>`

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

### `bvm uninstall <name-selector> <version>`

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

### `bvm use <name-selector> <version-selector>`

Uses the specified binary name and version globally.

The binary and version must have been previously installed.

```
# Examples
bvm use deno 1.3.2
bvm use denoland/deno 1.3.2
bvm use name-stealer/deno 2.0.0
bvm use deno 1
bvm use deno 1.0
bvm use deno "^1.1"
bvm use deno ~1.1.3
```

### `bvm use <name-selector> path`

Uses the version of the binary that's installed on the path if it exists.

```
# Example
bvm use deno path
```

### `bvm exec <name-selector> <version-selector> [command-name] [...args]`

Executes the version of the matching binary.

```
# Examples
bvm exec deno 1.3.1 -V
bvm exec deno path -v
bvm exec node "^12.1.1" -v
bvm exec node 14 npm -v
bvm exec nodejs/node ~8.2.1 -v
bvm exec node 14 npm install -g rimraf
bvm exec node 14 rimraf dir-to-delete
```

### `bvm clear-url-cache`

Clears any cached urls.

## Registry commands

Adding a registry allows you to more easily install copies of a binary without dealing with urls.

### `bvm registry add <url>`

Adds or associates the registry at the specified url to the local CLI.

```
# Examples
bvm registry add https://bvm.land/deno/registry.json
bvm registry add https://bvm.land/node/registry.json
```

### `bvm registry remove <url>`

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

### `bvm install <name-selector>`

Installs the latest non-pre-release version of the specified binary based on the CLI's registries.

```
# Examples
bvm install deno
bvm install --use node
```

### `bvm install <name-selector> <version-selector>`

Installs the specified binary and version based on the first matching version in the CLI's registries.

```
# Examples
bvm install deno 1.3.3
bvm install deno 1
bvm install deno 1.3
bvm install deno "^1.3.1"
bvm install --use node 14.9.0
```

## Projects

`bvm` allows for specifying versions of binaries to automatically use within a directory.

### Setup

1. Run `bvm init` in the project's root directory.
2. Open up the created _bvm.json_ file (or optionally rename it as hidden first—`_.bvm.json`) and specify the paths to the binary manifest files.
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

Creates an empty `bvm.json` file in the current directory.

### `bvm install`

Downloads & installs the binaries in the current bvm configuration file and associates them on the path with bvm if not previously done.

- Provide the `--use` flag to also use all the binaries in the configuration file on the path when outside this directory.
- Provide the `--force` flag to force an install of everything even if already installed or has a matching version.

### `bvm add [url]`

Adds the specified binary at the specified url to a project's bvm configuration file based on the current directory. Installs if necessary.

#### Example

```
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

### `bvm add [binary-name or owner-name/binary-name] [version-selector]`

Similar to above with a url, but adds the specified binary from an added registry.

The version is optional.

```
# Examples
bvm add deno 1.3.1
bvm add deno ~1.3.1
bvm add node
```

### `bvm use`

Uses all the binaries in the current configuration files globally on the path.

Generally it's not necessary to ever use this command as this happens automatically being in the current directory.

## bvm.land

The website https://bvm.land is not required to use with bvm, but it provides some services that makes publishing binaries a little easier.

### Redirect Service

If you publish a _bvm.json_ file as a GitHub release asset (not recommended yet, due to this being a proof of concept) then you can use `https://bvm.land` to redirect to your release.

1. `https://bvm.land/<owner>/<name>/<release-tag>.json` -> `https://github.com/<owner>/<name>/releases/download/<release-tag>/bvm.json`
2. `https://bvm.land/<name>/<release-tag>.json` -> `https://github.com/<name>/<name>/releases/download/<release-tag>/bvm.json`

Example: `https://bvm.land/dprint/0.9.1.json`

### Automatic registry file creation

The bvm.land server will create _registry.json_ files when requested. These files can then be used with the `bvm registry` sub command.

To cause the server to update a registry file, make a `GET` request to `https://bvm.land/refresh-registry/<owner>/<repo-name>`. After a few minutes, you should have a registry file created at either of the two endpoints depending on your repo owner and name:

1. `https://bvm.land/<owner>/<name>/registry.json`
2. `https://bvm.land/<name>/registry.json`—when `owner` is the same as `name`.

The file will be created based on any releases containing a _bvm.json_ file as a release asset as described above.

#### Example GitHub Publish Workflow

In _.github/workflows/publish.yml_:

```
name: Publish

on:
  release:
    types: [published]
jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
    - name: Update registry.json file at https://bvm.land/dprint/registry.json
      run: curl -s -o /dev/null -v https://bvm.land/refresh-registry/dprint/dprint
```

This will cause the _registry.json_ file on bvm.land to be updated after clicking "publish" on a GitHub release.

## Binary manifest file

At the moment, it looks like this:

```jsonc
{
  "schemaVersion": 1,
  "name": "deno",
  "owner": "denoland",
  "description": "A secure JavaScript and TypeScript runtime.",
  "version": "1.4.4",
  "windows-x86_64": {
    "path": "https://github.com/denoland/deno/releases/download/v1.4.4/deno-x86_64-pc-windows-msvc.zip",
    "type": "zip",
    "checksum": "3013f3dd2f96a6748461de2221e102f58f6b6f8dc392ca89a0968b05a79e1325",
    "commands": [
      {
        "name": "deno",
        "path": "bin/deno.exe"
      }
    ],
    "outputDir": "bin",
    "environment": {
      "path": [
        // Any local paths that should be added to the environment
        // when this is used or executed.
      ],
      "variables": {
        "DENO_INSTALL_ROOT": "%BVM_CURRENT_BINARY_DIR%"
      }
    },
    "onPreInstall": "", // command to run before installation
    "onPostInstall": "" // command to run after installation
  },
  "linux-x86_64": {
    "path": "https://github.com/denoland/deno/releases/download/v1.4.4/deno-x86_64-unknown-linux-gnu.zip",
    "type": "zip",
    "checksum": "ce2ad2e51b3b49a4d7844fa26092437eaaa89e90e2df4bf33859b9fb8c89be9c",
    "commands": [
      {
        "name": "deno",
        "path": "bin/deno"
      }
    ],
    "outputDir": "bin",
    "environment": {
      "variables": {
        "DENO_INSTALL_ROOT": "$BVM_CURRENT_BINARY_DIR"
      }
    }
  },
  "darwin-x86_64": {
    "path": "https://github.com/denoland/deno/releases/download/v1.4.4/deno-x86_64-apple-darwin.zip",
    "type": "zip",
    "checksum": "fd8997040dcfc6ef48ef4b05c88b1a8b30362c03ebb552a23a7888bcc60b77a0",
    "commands": [
      {
        "name": "deno",
        "path": "bin/deno"
      }
    ],
    "outputDir": "bin",
    "environment": {
      "variables": {
        "DENO_INSTALL_ROOT": "$BVM_CURRENT_BINARY_DIR"
      }
    }
  }
}
```

Supported types: `zip`, `exe`, `tar.gz` (will add more later)

Other examples:

- Multiple commands: [https://bvm.land/node/14.9.0.json](https://bvm.land/node/14.9.0.json)
