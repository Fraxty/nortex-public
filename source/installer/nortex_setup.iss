; Inno Setup 6 - Eingabe: dist\staging (siehe scripts\stage_release.ps1)
#define AppName "NORTEX Code"
#define AppVersion "0.1.0"
#define AppPublisher "NORTEX"
#define AppExeName "bin\nortex-code.exe"

[Setup]
AppId={{8A1F3B2C-4D9E-4F8A-9B3C-1D2E3F4A5B6C}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\NORTEX Code
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DefaultGroupName={#AppName}
AllowNoIcons=yes
OutputDir=..\dist
OutputBaseFilename=NORTEX-Setup-x64
SetupIconFile=..\assets\icons\app\nortex-code.ico
UninstallDisplayIcon={app}\{#AppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesAssociations=yes
ChangesEnvironment=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "assoc"; Description: ".nx-Dateien mit NORTEX Code öffnen"; GroupDescription: "Dateiverknüpfung:"
Name: "addtopath"; Description: "Installationsordner \bin zur PATH-Umgebungsvariable hinzufügen (nortexc überall aufrufbar)"; GroupDescription: "Umgebung:"; Flags: unchecked

[Files]
Source: "..\dist\staging\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Registry]
Root: HKA; Subkey: "Software\Classes\.nx"; ValueType: string; ValueName: ""; ValueData: "Nortex.Source"; Flags: uninsdeletevalue uninsdeletekeyifempty; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Nortex.Source"; ValueType: string; ValueName: ""; ValueData: "NORTEX Quelltext"; Flags: uninsdeletekey; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Nortex.Source\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExeName},0"; Tasks: assoc
Root: HKA; Subkey: "Software\Classes\Nortex.Source\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExeName}"" ""%1"""; Tasks: assoc

[Run]
Filename: "{app}\{#AppExeName}"; Description: "NORTEX Code starten"; Flags: nowait postinstall skipifsilent

[Code]
function BinDir(): string;
begin
  Result := ExpandConstant('{app}\bin');
end;

function PathContains(const Path, Dir: string): Boolean;
begin
  Result := Pos(';' + Lowercase(Dir) + ';', ';' + Lowercase(Path) + ';') > 0;
end;

function EnvRoot(): Integer;
begin
  if IsAdminInstallMode() then
    Result := HKLM
  else
    Result := HKCU;
end;

function EnvSubkey(): string;
begin
  if IsAdminInstallMode() then
    Result := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
  else
    Result := 'Environment';
end;

procedure AddToPath();
var
  Path: string;
begin
  if not RegQueryStringValue(EnvRoot(), EnvSubkey(), 'Path', Path) then
    Path := '';
  if PathContains(Path, BinDir()) then
    Exit;
  if (Path <> '') and (Path[Length(Path)] <> ';') then
    Path := Path + ';';
  RegWriteExpandStringValue(EnvRoot(), EnvSubkey(), 'Path', Path + BinDir());
end;

procedure RemoveFromPath();
var
  Path, Needle: string;
  P: Integer;
begin
  if not RegQueryStringValue(EnvRoot(), EnvSubkey(), 'Path', Path) then
    Exit;
  Needle := BinDir();
  P := Pos(';' + Lowercase(Needle), ';' + Lowercase(Path));
  if P = 0 then
    Exit;
  Delete(Path, P, Length(Needle) + 1);
  RegWriteExpandStringValue(EnvRoot(), EnvSubkey(), 'Path', Path);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and WizardIsTaskSelected('addtopath') then
    AddToPath();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath();
end;
