; Inno Setup Script for Liem Wallpaper
; Compile this script using Inno Setup (ISCC) to generate a professional wizard installer.

[Setup]
AppName=Liem Wallpaper
AppVersion=0.1.2
AppPublisher=Liem Wallpaper Contributors
DefaultDirName={userpf}\Liem Wallpaper
DefaultGroupName=Liem Wallpaper
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\lw-service.exe
Compression=lzma2
SolidCompression=yes
OutputDir=target\installer
OutputBaseFilename=LiemWallpaperSetup
ChangesEnvironment=yes

[Files]
Source: "target\release\lw-service.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\lw.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "shaders\*"; DestDir: "{app}\shaders"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Liem Wallpaper Service"; Filename: "{app}\lw-service.exe"

[Run]
; Spawn service after successful installation
Filename: "{app}\lw-service.exe"; Description: "Start Liem Wallpaper Service"; Flags: nowait postinstall runhidden

[Registry]
; Add installation directory to User PATH
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath

[Code]
procedure CleanOldPath();
var
  OldInstallDir: String;
  Path: String;
  PosOldDir: Integer;
begin
  if RegQueryStringValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Uninstall\LiemWallpaper', 'InstallLocation', OldInstallDir) then
  begin
    if OldInstallDir <> '' then
    begin
      if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path) then
      begin
        PosOldDir := Pos(';' + Uppercase(OldInstallDir), Uppercase(Path));
        if PosOldDir > 0 then
        begin
          Delete(Path, PosOldDir, Length(';' + OldInstallDir));
          RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path);
        end;
        
        PosOldDir := Pos(Uppercase(OldInstallDir) + ';', Uppercase(Path));
        if PosOldDir > 0 then
        begin
          Delete(Path, PosOldDir, Length(OldInstallDir + ';'));
          RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path);
        end;
        
        PosOldDir := Pos(Uppercase(OldInstallDir), Uppercase(Path));
        if PosOldDir > 0 then
        begin
          Delete(Path, PosOldDir, Length(OldInstallDir));
          RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path);
        end;
      end;
    end;
  end;
end;

function InitializeSetup(): Boolean;
var
  ResultCode: Integer;
begin
  Result := True;
  CleanOldPath();
  Exec('taskkill.exe', '/F /IM lw-service.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function NeedsAddPath(): Boolean;
var
  Path: String;
  AppDir: String;
begin
  AppDir := ExpandConstant('{app}');
  if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path) then
  begin
    Result := Pos(Uppercase(AppDir), Uppercase(Path)) = 0;
  end
  else
  begin
    Result := True;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Path: String;
  AppDir: String;
  PosAppDir: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    // Remove path from environment on uninstall
    AppDir := ';' + ExpandConstant('{app}');
    if RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path) then
    begin
      PosAppDir := Pos(Uppercase(AppDir), Uppercase(Path));
      if PosAppDir > 0 then
      begin
        Delete(Path, PosAppDir, Length(AppDir));
        RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Path);
      end;
    end;
  end;
end;
