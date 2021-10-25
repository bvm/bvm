#!/usr/bin/env pwsh

$bvm_bin = Join-Path -Path $PSScriptRoot -ChildPath "bvm-bin"

function bvm_handle_env_messages {
  param(
    [string]
    $messages_text,
    [object[]]
    $exec_args
  )

  $items = $messages_text -split [Environment]::NewLine
  foreach ($item in $items) {
    if ([string]::IsNullOrWhitespace($item)) {
        # ignore
    } elseif ($last_was_exec -eq 1) {
      # this item will be the binary to execute
      . $item @exec_args # splat the arguments
      if ($lastexitcode -ne 0) { exit $lastexitcode }
    } elseif ($item -match "^SET ([^\s]+)=(.+)$") {
      # adding env var
      $name = $Matches.1
      Set-Item "env:$name" $Matches.2
    } elseif ($item -match "^SET ([^\s]+)=$") {
      # removing env var
      $name = $Matches.1
      Remove-Item "env:$name" -ErrorAction SilentlyContinue
    } elseif ($item.trim() -eq "EXEC") {
      # set that the next item should be the binary that's executed
      $last_was_exec=1
    } else {
      throw "Internal error in bvm. Unhandled line: $item"
    }
  }
}

function has_env_changes {
    param([string]$messages_text)
    $messages_text = $messages_text.trim()
    $new_line = [Environment]::NewLine
    if (($messages_text -eq $null) -or ($messages_text.Length -eq 0) -or ($messages_text.StartsWith("EXEC$new_line"))) {
        return 0
    } else {
        return 1
    }
}

# couldn't figure out sub shells in powershell, so this is snapshotting the environment variables then restoring them after running the command
function snapshot_env {
    $snapshot = [System.Collections.ArrayList]::new()
    $items=(dir env:)
    foreach ($item in $items) {
        if (($item.Name -ne $null) -and ($item.Value -ne $null)) {
            $snapshot.Add([System.Tuple]::Create($item.Name, $item.Value)) | Out-Null
        }
    }
    return $snapshot
}

function restore_env {
    param($snapshot)
    $items=(dir env:)
    # delete all the current environment variables
    foreach ($item in $items) {
        $name=$item.Name
        Remove-Item "env:$name" -ErrorAction SilentlyContinue
    }
    # now restore from the snapshot
    foreach ($item in $snapshot) {
        $name=$item.Item1
        $value=$item.Item2
        Set-Item "env:$name" $value
    }
}

function process_args {
  $newArgs=@()
  foreach ($arg in $args) {
    if ($arg -is [array]) {
      # collapse back any comma separated arguments to a string
      $newArgs += $arg -join ","
    } else {
      $newArgs += $arg
    }
  }
  return ,$newArgs
}

$args=(process_args @args)

if ($args[0] -eq "exec-command") {
  # Format: bvm exec-command [command-name] [...args]
  $command_name = $args[1]
  $fallback_path = $args[2]
  $exec_args = $args[3..$args.Length]
  # todo: windows specific behaviour
  if (($env:USERNAME -eq "") -or ($env:USERNAME -eq $null)) {
    . $fallback_path @exec_args # splat the arguments
  } else {
    $env_messages=((. $bvm_bin hidden resolve-command $command_name) | Out-String)
    $should_snapshot_env=(has_env_changes $env_messages)
    if ($should_snapshot_env -eq 1) { $env_snapshot=(snapshot_env) }
    try {
      bvm_handle_env_messages $env_messages $exec_args
    } finally {
      if ($should_snapshot_env -eq 1) { restore_env $env_snapshot }
    }
  }
  if ($lastexitcode -ne 0) { exit $lastexitcode }
} elseif ($args[0] -eq "exec") {
  # Format: bvm exec [name-selector] [version-selector] [command-name] [...args]
  $exec_name = $args[1]
  $exec_version = $args[2]
  $exec_command = $args[3]
  $args_start_index = 4

  $has_command = ((. $bvm_bin hidden has-command $exec_name $exec_version $exec_command) | Out-String).trim()
  if ($lastexitcode -ne 0) { exit $lastexitcode }

  if ($has_command -eq "false") {
     $exec_command = $exec_name
     $args_start_index = 3
  }

  $executable_path=((. $bvm_bin hidden get-exec-command-path $exec_name $exec_version $exec_command) | Out-String).trim()
  if ($lastexitcode -ne 0) { exit $lastexitcode }

  $env_messages=((. $bvm_bin hidden get-exec-env-changes $exec_name $exec_version) | Out-String)
  $should_snapshot_env=(has_env_changes $env_messages)
  if ($should_snapshot_env -eq 1) { $env_snapshot=(snapshot_env) }
  try {
    bvm_handle_env_messages $env_messages
    $args = $args[$args_start_index..$args.Length]
    . $executable_path @args # splat
    if ($lastexitcode -ne 0) { exit $lastexitcode }
  } finally {
    if ($should_snapshot_env -eq 1) { restore_env $env_snapshot }
  }
} else {
  . $bvm_bin @args # splat

  if (($args[0] -eq "install") -or ($args[0] -eq "uninstall") -or ($args[0] -eq "use")) {
    bvm_handle_env_messages ((. $bvm_bin hidden get-pending-env-changes) | Out-String)
    if ($lastexitcode -ne 0) { exit $lastexitcode }
    . $bvm_bin hidden clear-pending-env-changes
    if ($lastexitcode -ne 0) { exit $lastexitcode }
  }
}
