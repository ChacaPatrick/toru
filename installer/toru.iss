; Toru's installer. Build it with build.ps1, which builds the release exe first and passes
; AppVersion from Cargo.toml.
;
; Per-user: no admin rights, installs to %LOCALAPPDATA%\Programs\Toru. Unsigned, so Windows
; SmartScreen warns on first run (README.md).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "Toru"
#define AppExe "toru.exe"
#define RunKey "Software\Microsoft\Windows\CurrentVersion\Run"
; Where Windows' Startup apps page marks the Run value disabled without removing it.
#define ApprovedKey "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
; The app writes and reads the same value under both (crates/toru/src/platform.rs, RUN_VALUE).
#define RunValue "Toru"
; Toru holds this mutex while it runs (crates/toru/src/platform.rs, mark_running).
#define AppMutexName "Toru.Running"

[Setup]
AppId={{C2230C4F-84D6-4519-A025-3B91F6D339AF}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Toru
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Toru holds this mutex while it runs (platform::mark_running), so Setup and the uninstaller ask
; for it to be quit first; quitting from the tray applies the On close setting.
; An update (/UPDATE=1) waits for it first instead (InitializeSetup below).
AppMutex={#AppMutexName}
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir=output
OutputBaseFilename=Toru-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Every install starts from the defaults below, so an upgrade (silent or not) never re-ticks
; "Start Toru when Windows starts" after the user turned it off in Toru. Leaving it unticked
; changes nothing: the app's own setting stays as it is.
UsePreviousTasks=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startup"; Description: "Start Toru when Windows starts"; GroupDescription: "Start-up:"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
; Toru's own licence (MIT).
Source: "..\LICENSE"; DestDir: "{app}\licences"; DestName: "LICENSE.txt"; Flags: ignoreversion
; The fonts are compiled into the exe; their licences ship beside it.
Source: "..\assets\fonts\*-OFL.txt"; DestDir: "{app}\licences\fonts"; Flags: ignoreversion
Source: "..\assets\fonts\README.md"; DestDir: "{app}\licences\fonts"; Flags: ignoreversion
; The licence notices of the crates compiled into the exe, written by build.ps1 with cargo-about.
Source: "..\target\THIRD-PARTY-NOTICES.html"; DestDir: "{app}\licences"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; The same value the app's Start with Windows setting writes, so either can turn it off.
Root: HKCU; Subkey: "{#RunKey}"; ValueType: string; ValueName: "{#RunValue}"; ValueData: """{app}\{#AppExe}"" --tray"; Tasks: startup
; As the app does when its switch is turned on, so an earlier "disabled" there doesn't win.
Root: HKCU; Subkey: "{#ApprovedKey}"; ValueType: none; ValueName: "{#RunValue}"; Flags: deletevalue; Tasks: startup

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
; An in-app update runs Setup silently with /UPDATE=1 as Toru exits; start the new version.
Filename: "{app}\{#AppExe}"; Flags: nowait skipifnotsilent; Check: IsUpdate

[Code]
// Toru starts `Toru-<version>-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /UPDATE=1`
// right as it exits.
function IsUpdate(): Boolean;
begin
  Result := ExpandConstant('{param:UPDATE|0}') = '1';
end;

// Setup checks AppMutex once, straight after InitializeSetup, and a silent Setup answers the
// "Toru is running" prompt with Cancel. So an update gives the exiting Toru up to 30 seconds to
// let go of the mutex first, and gives up quietly, installing nothing, if it doesn't.
function InitializeSetup(): Boolean;
var
  Waited: Integer;
begin
  Result := True;
  if not IsUpdate() then
    Exit;
  Waited := 0;
  while CheckForMutexes('{#AppMutexName}') and (Waited < 30000) do
  begin
    Sleep(250);
    Waited := Waited + 250;
  end;
  if CheckForMutexes('{#AppMutexName}') then
  begin
    Log('Toru is still running after 30 seconds; not updating.');
    Result := False;
  end;
end;

// The store's own files in %LOCALAPPDATA%\Toru (toru-core's Store). Only these go, so anything
// else in the folder, such as a save folder the user pointed there, stays.
procedure DeleteToruData();
var
  Data: String;
begin
  Data := ExpandConstant('{localappdata}\Toru');
  DelTree(Data + '\temp', True, True, True);
  DelTree(Data + '\copies', True, True, True);
  DeleteFile(Data + '\history.sqlite3');
  DeleteFile(Data + '\history.sqlite3-journal');
  DeleteFile(Data + '\history.sqlite3-wal');
  DeleteFile(Data + '\history.sqlite3-shm');
  DeleteFile(Data + '\store.lock');
  // Only removed if nothing else is left in it.
  RemoveDir(Data);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    // Written by the startup task or by the app's setting; either way it would point at a
    // removed exe.
    RegDeleteValue(HKEY_CURRENT_USER, '{#RunKey}', '{#RunValue}');
    RegDeleteValue(HKEY_CURRENT_USER, '{#ApprovedKey}', '{#RunValue}');
  end;
  if CurUninstallStep = usPostUninstall then
  begin
    // Saved screenshots live in the user's save folder and are never touched. History and
    // unsaved captures are kept unless the user says otherwise; a silent uninstall keeps them.
    if DirExists(ExpandConstant('{localappdata}\Toru')) and not UninstallSilent() then
    begin
      if MsgBox('Also delete Toru''s history and unsaved captures?' + #13#10#13#10 +
                'They are in ' + ExpandConstant('{localappdata}\Toru') + '. ' +
                'Screenshots you saved are not touched either way.',
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DeleteToruData();
    end;
  end;
end;
