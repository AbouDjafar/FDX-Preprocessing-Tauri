#ifndef AppSource
  #define AppSource "..\dist\FDXAtelier.dist"
#endif

[Setup]
AppId={{C15E7ECA-9337-4321-88A8-049EBC758E20}
AppName=FDX Atelier Donnees
AppVersion=1.1.0
AppPublisher=FDX
DefaultDirName={localappdata}\Programs\FDXAtelier
DefaultGroupName=FDX Atelier Donnees
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=FDXAtelier-Installation-1.1.0
SetupIconFile=..\assets\fdx.ico
UninstallDisplayIcon={app}\FDXAtelier.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
CloseApplications=yes

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Tasks]
Name: "desktopicon"; Description: "Creer un raccourci sur le Bureau"; Flags: unchecked

[Files]
Source: "{#AppSource}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\FDX Atelier Donnees"; Filename: "{app}\FDXAtelier.exe"
Name: "{autodesktop}\FDX Atelier Donnees"; Filename: "{app}\FDXAtelier.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\FDXAtelier.exe"; Description: "Ouvrir FDX Atelier Donnees"; Flags: nowait postinstall skipifsilent

