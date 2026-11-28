; The Windows installer (Inno Setup). Built by .github/workflows/release.yml:
;   iscc /DVersion=1.0.0 /DSource=..\..\target\release packaging\windows\rippl.iss
; Installs for you alone (no administrator needed) into
; %LOCALAPPDATA%\Programs\rippl, adds rippl to Start and to Default apps'
; browser list, and gets Microsoft's WebView2 runtime if it is missing
; (Windows 11 and up-to-date Windows 10 have it already).

#ifndef Version
  #define Version "1.0.0"
#endif
#ifndef Source
  #define Source "..\..\target\release"
#endif

[Setup]
AppId={{7C1E2A57-4B5D-4C8E-9A1F-52B0E3D1A9C4}
AppName=rippl
AppVersion={#Version}
AppPublisher=rippl
AppPublisherURL=https://ripplbrowser.web.app
AppSupportURL=https://github.com/amxnn05/rippl
DefaultDirName={localappdata}\Programs\rippl
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
OutputDir=..\..\dist
OutputBaseFilename=rippl-windows-x64-setup
SetupIconFile=rippl.ico
UninstallDisplayIcon={app}\rippl.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes

[Files]
Source: "{#Source}\rippl.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "rippl.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\rippl"; Filename: "{app}\rippl.exe"; IconFilename: "{app}\rippl.ico"
Name: "{userdesktop}\rippl"; Filename: "{app}\rippl.exe"; IconFilename: "{app}\rippl.ico"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "A shortcut on the desktop"; Flags: unchecked

[Registry]
; rippl as a browser Windows can offer in Default apps.
Root: HKCU; Subkey: "Software\Classes\ripplHTML"; ValueType: string; ValueData: "rippl HTML Document"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\ripplHTML\DefaultIcon"; ValueType: string; ValueData: "{app}\rippl.ico"
Root: HKCU; Subkey: "Software\Classes\ripplHTML\shell\open\command"; ValueType: string; ValueData: """{app}\rippl.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl"; ValueType: string; ValueData: "rippl"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl\DefaultIcon"; ValueType: string; ValueData: "{app}\rippl.ico"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl\shell\open\command"; ValueType: string; ValueData: """{app}\rippl.exe"""
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "rippl"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "A small, fast, quiet web browser"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl\Capabilities\URLAssociations"; ValueType: string; ValueName: "http"; ValueData: "ripplHTML"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl\Capabilities\URLAssociations"; ValueType: string; ValueName: "https"; ValueData: "ripplHTML"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl\Capabilities\FileAssociations"; ValueType: string; ValueName: ".html"; ValueData: "ripplHTML"
Root: HKCU; Subkey: "Software\Clients\StartMenuInternet\rippl\Capabilities\FileAssociations"; ValueType: string; ValueName: ".htm"; ValueData: "ripplHTML"
Root: HKCU; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "rippl"; ValueData: "Software\Clients\StartMenuInternet\rippl\Capabilities"; Flags: uninsdeletevalue

[Run]
Filename: "{tmp}\MicrosoftEdgeWebview2Setup.exe"; Parameters: "/silent /install"; StatusMsg: "Getting Microsoft's WebView2…"; Check: NeedsWebView2; Flags: waituntilterminated
Filename: "{app}\rippl.exe"; Description: "Open rippl"; Flags: nowait postinstall skipifsilent

[Code]
function NeedsWebView2: Boolean;
var v: String;
begin
  Result := not (RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', v) and (v <> '') and (v <> '0.0.0.0'))
    and not (RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', v) and (v <> '') and (v <> '0.0.0.0'));
  if Result then
    DownloadTemporaryFile('https://go.microsoft.com/fwlink/p/?LinkId=2124703', 'MicrosoftEdgeWebview2Setup.exe', '', nil);
end;
