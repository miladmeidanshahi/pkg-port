#ifndef AppVersion
#define AppVersion "0.1.1"
#endif

[Setup]
AppId={{E496F1C9-BE27-4CCA-8062-708E1B06D019}
AppName=PKGPort
AppVersion={#AppVersion}
AppPublisher=PKGPort contributors
DefaultDirName={localappdata}\Programs\PKGPort
DefaultGroupName=PKGPort
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
UsePreviousAppDir=yes
CloseApplications=yes
UninstallDisplayIcon={app}\pkgport.exe
SetupIconFile=..\assets\icon.ico
OutputDir=..\dist
OutputBaseFilename=pkgport-{#AppVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Files]
Source: "..\target\release\pkgport.exe"; DestDir: "{app}"; Flags: ignoreversion

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"; Flags: unchecked

[Icons]
Name: "{autoprograms}\PKGPort"; Filename: "{app}\pkgport.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\PKGPort"; Filename: "{app}\pkgport.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\pkgport.exe"; Description: "Launch PKGPort"; Flags: nowait postinstall skipifsilent; WorkingDir: "{app}"
