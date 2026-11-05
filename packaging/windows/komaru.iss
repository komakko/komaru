#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef SourceExe
  #define SourceExe "..\..\target\x86_64-pc-windows-msvc\release\komaru.exe"
#endif

[Setup]
AppId={{7B07E68D-DF47-49F2-876C-CE028ED4DC7A}
AppName=Komaru
AppVersion={#AppVersion}
AppPublisher=komakko
AppPublisherURL=https://github.com/komakko/komaru
DefaultDirName={autopf}\komaru
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesAssociations=yes
CloseApplications=yes
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\..\LICENSE
SetupIconFile=..\..\assets\icon\komaru.ico
UninstallDisplayIcon={app}\komaru.exe
UninstallDisplayName=Komaru
OutputDir=..\..\dist
OutputBaseFilename=komaru-x86_64-windows-setup

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "komaru.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Komaru"; Filename: "{app}\komaru.exe"

[Registry]
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Komaru"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe\shell\open\command"; ValueType: string; ValueData: """{app}\komaru.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe\SupportedTypes"; ValueType: string; ValueName: ".jpg"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe\SupportedTypes"; ValueType: string; ValueName: ".jpeg"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe\SupportedTypes"; ValueType: string; ValueName: ".png"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe\SupportedTypes"; ValueType: string; ValueName: ".webp"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe\SupportedTypes"; ValueType: string; ValueName: ".gif"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe\SupportedTypes"; ValueType: string; ValueName: ".bmp"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\komaru.exe\SupportedTypes"; ValueType: string; ValueName: ".avif"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\komaru.image"; ValueType: string; ValueData: "Komaru image"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\komaru.image\DefaultIcon"; ValueType: string; ValueData: "{app}\komaru.exe,0"
Root: HKCU; Subkey: "Software\Classes\komaru.image\shell\open\command"; ValueType: string; ValueData: """{app}\komaru.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\.jpg\OpenWithProgids"; ValueType: string; ValueName: "komaru.image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.jpeg\OpenWithProgids"; ValueType: string; ValueName: "komaru.image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.png\OpenWithProgids"; ValueType: string; ValueName: "komaru.image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.webp\OpenWithProgids"; ValueType: string; ValueName: "komaru.image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.gif\OpenWithProgids"; ValueType: string; ValueName: "komaru.image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.bmp\OpenWithProgids"; ValueType: string; ValueName: "komaru.image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Classes\.avif\OpenWithProgids"; ValueType: string; ValueName: "komaru.image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Komaru\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "Komaru"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Komaru\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Fast reference image viewer"
Root: HKCU; Subkey: "Software\Komaru\Capabilities\FileAssociations"; ValueType: string; ValueName: ".jpg"; ValueData: "komaru.image"
Root: HKCU; Subkey: "Software\Komaru\Capabilities\FileAssociations"; ValueType: string; ValueName: ".jpeg"; ValueData: "komaru.image"
Root: HKCU; Subkey: "Software\Komaru\Capabilities\FileAssociations"; ValueType: string; ValueName: ".png"; ValueData: "komaru.image"
Root: HKCU; Subkey: "Software\Komaru\Capabilities\FileAssociations"; ValueType: string; ValueName: ".webp"; ValueData: "komaru.image"
Root: HKCU; Subkey: "Software\Komaru\Capabilities\FileAssociations"; ValueType: string; ValueName: ".gif"; ValueData: "komaru.image"
Root: HKCU; Subkey: "Software\Komaru\Capabilities\FileAssociations"; ValueType: string; ValueName: ".bmp"; ValueData: "komaru.image"
Root: HKCU; Subkey: "Software\Komaru\Capabilities\FileAssociations"; ValueType: string; ValueName: ".avif"; ValueData: "komaru.image"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "Komaru"; ValueData: "Software\Komaru\Capabilities"; Flags: uninsdeletevalue
