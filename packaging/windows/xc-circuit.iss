; The Windows installer, made with Inno Setup 6.3 or newer:
;   iscc /DAppVersion=0.1.0 /DArch=x64 /DSourceDir=..\..\XC-Circuit-windows-x64 /DOutputDir=..\..\dist xc-circuit.iss
; Arch is x64 or arm64. SourceDir is the unpacked release folder.
;
; It installs for the current user by default (no administrator needed);
; the first page offers installing for everyone instead.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\XC-Circuit-windows-" + Arch
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
; Never change the AppId: Windows recognises updates and the uninstaller by it.
AppId={{6E1E3A52-0C4F-4D2B-9C8E-5A7B3F0D21C7}
AppName=XC™ Circuit
AppVersion={#AppVersion}
AppVerName=XC™ Circuit {#AppVersion}
AppPublisher=XCraftTM
AppPublisherURL=https://github.com/XCraftTM
AppSupportURL=https://github.com/XCraftTM/XC-Circuit/issues
AppUpdatesURL=https://github.com/XCraftTM/XC-Circuit/releases
AppCopyright=Copyright (c) 2026 XCraftTM
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\XC Circuit
DefaultGroupName=XC Circuit
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
LicenseFile={#SourceDir}\LICENSE
SetupIconFile=..\icons\xc-circuit.ico
UninstallDisplayIcon={app}\xc-circuit.exe
UninstallDisplayName=XC™ Circuit
OutputDir={#OutputDir}
OutputBaseFilename=XC-Circuit-{#AppVersion}-windows-{#Arch}-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
MinVersion=10.0
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
; Close a running XC Circuit before updating it.
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

; An update replaces the bundled plugins and examples completely, so files
; removed in the new version don't linger. Your own plugins, projects and
; settings live elsewhere and are never touched.
[InstallDelete]
Type: filesandordirs; Name: "{app}\plugins"
Type: filesandordirs; Name: "{app}\examples"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{autoprograms}\XC Circuit"; Filename: "{app}\xc-circuit.exe"; Comment: "Build bots by connecting blocks"
Name: "{autodesktop}\XC Circuit"; Filename: "{app}\xc-circuit.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\xc-circuit.exe"; Description: "{cm:LaunchProgram,XC Circuit}"; Flags: nowait postinstall skipifsilent
; An update from inside the app runs this Setup silently with /restart=1:
; then XC Circuit opens again when it's done (as the user, not as admin).
Filename: "{app}\xc-circuit.exe"; Flags: nowait runasoriginaluser; Check: RestartWanted

[Code]
function RestartWanted: Boolean;
begin
  Result := ExpandConstant('{param:restart|0}') = '1';
end;
