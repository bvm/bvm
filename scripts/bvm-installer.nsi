# dprint installer script
# Copyright 2020 David Sherret. All rights reserved. MIT license.

Name "bvm"

RequestExecutionLevel User

OutFile "bvm-x86_64-pc-windows-msvc-installer.exe"
InstallDir $PROFILE\.bvm

!macro KillBvmProcess
    # https://stackoverflow.com/a/34371858/188246
    nsExec::ExecToStack `wmic Path win32_process where "name like 'bvm.exe'" Call Terminate`
    Pop $0 # return value
    Pop $1 # printed text
!macroend

Section

    !insertmacro KillBvmProcess

    # Create the executable files
    CreateDirectory $INSTDIR\bin
    SetOutPath $INSTDIR\bin
    File ..\target\release\bvm-bin.exe
    File ..\bvm.cmd
    SetOutPath $INSTDIR

    nsExec::Exec '$INSTDIR\bin\bvm-bin.exe hidden-shell windows-install "$INSTDIR\bin"'
    Pop $0

    WriteUninstaller $INSTDIR\uninstall.exe

    # Note: Don't bother adding to registry keys in order to do "Add/remove programs"
    # because we'd rather run the installer with `RequestExecutionLevel User`. We
    # tell the user in this message how to uninstall if they wish to do so.

    MessageBox MB_OK "Success! Installed to: $INSTDIR$\n$\nTo get started, restart your terminal and \
        run the following command:$\n$\n    bvm --help$\n$\nTo uninstall run: $INSTDIR\uninstall.exe"

SectionEnd

Section "Uninstall"

    !insertmacro KillBvmProcess

    nsExec::Exec '$INSTDIR\bin\bvm-bin.exe hidden-shell windows-uninstall "$INSTDIR\bin"'

    Delete $INSTDIR\uninstall.exe
    Delete $INSTDIR\bin\bvm-bin.exe
    Delete $INSTDIR\bin\bvm.cmd
    RMDir $INSTDIR\bin
    RMDir $INSTDIR

    # delete the plugin local cache folder
    RMDir /r $LOCALAPPDATA\bvm
SectionEnd
