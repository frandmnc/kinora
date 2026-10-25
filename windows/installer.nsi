; El instalador de Kinora para Windows: un asistente en castellano con la estética de Kinora.
; Instala para el usuario, sin permisos de administrador (como VS Code): Kinora.exe (la app de
; la bandeja) y kinora-server.exe en %LOCALAPPDATA%\Programs\Kinora, el acceso directo en el
; menú Inicio y ffmpeg con winget si falta. Solo la regla del firewall para la tele necesita
; administrador: se pide una vez, la primera. Si Kinora ya está instalado, lo actualiza: lo
; cierra, guarda una copia de la base de datos y reemplaza el programa.
;
; En silencio (/S), sin asistente, es como se actualiza Kinora solo desde el Admin; con /RESTART
; además lo vuelve a abrir al terminar.
;
; Lo arma windows/build.sh desde la Mac (makensis), con:
;   -DVERSION=0.2.0 -DSERVER=<kinora.exe del servidor> -DTRAY=<Kinora.exe> -DOUTFILE=<setup.exe>

Unicode true
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "Sections.nsh"
!include "x64.nsh"

!define FIREWALL_RULE 'name="Kinora" dir=in action=allow program="$INSTDIR\kinora-server.exe" enable=yes profile=any remoteip=localsubnet'

Name "Kinora"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\Kinora"
InstallDirRegKey HKCU "Software\Kinora" "InstallDir"
RequestExecutionLevel user
BrandingText "Kinora ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=0 "ProductName" "Kinora"
VIAddVersionKey /LANG=0 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "FileDescription" "Instalador de Kinora"
VIAddVersionKey /LANG=0 "LegalCopyright" "Kinora"

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Kinora"
!define UNINSTALLER "Desinstalar Kinora.exe"

; --- La estética: el ícono de Kinora y las imágenes de la sala con la luz apagada ---

!define MUI_ICON "assets\kinora.ico"
!define MUI_UNICON "assets\kinora.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "assets\nsis-welcome.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "assets\nsis-welcome.bmp"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "assets\nsis-header.bmp"
!define MUI_HEADERIMAGE_UNBITMAP "assets\nsis-header.bmp"
!define MUI_ABORTWARNING
!define MUI_COMPONENTSPAGE_NODESC

; --- Los pasos ---

!define MUI_WELCOMEPAGE_TITLE "Kinora ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "Kinora es el servidor de películas y series de la casa: las entrega a la tele y a la compu por la red local.$\r$\n$\r$\nEste asistente lo instala en esta PC. Si ya está instalado, lo actualiza: lo cierra, guarda una copia de la base de datos y reemplaza el programa. Las películas, las series y lo visto no se tocan.$\r$\n$\r$\nTocá Siguiente para seguir."
!insertmacro MUI_PAGE_WELCOME

!define MUI_PAGE_HEADER_TEXT "Qué instalar"
!define MUI_PAGE_HEADER_SUBTEXT "Kinora necesita ffmpeg para importar y convertir videos."
!define MUI_COMPONENTSPAGE_TEXT_TOP "ffmpeg se instala con winget, el instalador de programas de Windows. Si ya lo tenés, no hace falta."
!insertmacro MUI_PAGE_COMPONENTS

!define MUI_PAGE_HEADER_TEXT "Dónde instalar"
!define MUI_PAGE_HEADER_SUBTEXT "Acá va el programa. La base de datos queda en tu carpeta de usuario (.kinora), y las películas y series, donde elijas en el Admin."
!insertmacro MUI_PAGE_DIRECTORY

!insertmacro MUI_PAGE_INSTFILES

!define MUI_FINISHPAGE_TITLE "Kinora está instalado"
!define MUI_FINISHPAGE_TEXT "Kinora queda en el menú Inicio. Al abrirlo aparece su ícono en la bandeja, al lado del reloj: desde ahí se abre el Player, el Admin y se copia la dirección para la tele.$\r$\n$\r$\nLa primera vez se abre el Admin para crear la cuenta y elegir la carpeta de las películas y series."
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Abrir Kinora"
!define MUI_FINISHPAGE_RUN_FUNCTION OpenKinora
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "Kinora se desinstaló"
!define MUI_FINISHPAGE_TEXT "Tus películas, tus series y la base de datos de Kinora (con los perfiles y lo visto) quedan donde estaban: la base, en la carpeta .kinora de tu usuario. Si volvés a instalar Kinora, sigue todo igual."
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "SpanishInternational"

Var PreviousVersion

; --- Lo que hace ---

Section "-Cerrar Kinora"
  ; Si está abierto, se le pide que se cierre (apaga el servidor ordenado) y se espera.
  ${If} ${FileExists} "$INSTDIR\Kinora.exe"
    DetailPrint "Cerrando Kinora…"
    ExecWait '"$INSTDIR\Kinora.exe" --quit'
  ${EndIf}
  ; Al actualizar, una copia de la base: la versión nueva puede cambiar su estructura al arrancar.
  ${If} $PreviousVersion != ""
  ${AndIf} $PreviousVersion != "${VERSION}"
  ${AndIf} ${FileExists} "$PROFILE\.kinora\kinora.db"
    DetailPrint "Guardando una copia de la base de datos (kinora.db.antes-de-${VERSION})…"
    CopyFiles /SILENT "$PROFILE\.kinora\kinora.db" "$PROFILE\.kinora\kinora.db.antes-de-${VERSION}"
    ${If} ${FileExists} "$PROFILE\.kinora\kinora.db-wal"
      CopyFiles /SILENT "$PROFILE\.kinora\kinora.db-wal" "$PROFILE\.kinora\kinora.db.antes-de-${VERSION}-wal"
    ${EndIf}
  ${EndIf}
SectionEnd

Section "Kinora" SecKinora
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "/oname=Kinora.exe" "${TRAY}"
  File "/oname=kinora-server.exe" "${SERVER}"
  WriteUninstaller "$INSTDIR\${UNINSTALLER}"

  CreateShortcut "$SMPROGRAMS\Kinora.lnk" "$INSTDIR\Kinora.exe" "" "$INSTDIR\Kinora.exe" 0

  ; El puerto para la tele: solo desde la red local, sea cual sea el tipo de red en Windows. Es
  ; lo único que necesita administrador: si la regla ya está (las actualizaciones), no se pide.
  nsExec::ExecToStack 'netsh advfirewall firewall show rule name="Kinora"'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Abriendo el puerto de Kinora en el firewall (Windows pide permiso una vez)…"
    ExecShellWait "runas" "$SYSDIR\netsh.exe" 'advfirewall firewall add rule ${FIREWALL_RULE}' SW_HIDE
  ${EndIf}

  ; Para "Agregar o quitar programas".
  WriteRegStr HKCU "Software\Kinora" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Kinora" "Version" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "Kinora"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Kinora"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\Kinora.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\${UNINSTALLER}"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\${UNINSTALLER}" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "ffmpeg (con winget)" SecFfmpeg
  DetailPrint "Instalando ffmpeg con winget (puede tardar unos minutos)…"
  nsExec::ExecToLog 'winget install --id Gyan.FFmpeg --exact --scope user --silent --accept-source-agreements --accept-package-agreements --disable-interactivity'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONINFORMATION "No se pudo instalar ffmpeg con winget (código $0).$\r$\n$\r$\nKinora se instala igual, pero para importar y convertir videos hace falta ffmpeg. Se puede instalar después, en una terminal:$\r$\n$\r$\n    winget install Gyan.FFmpeg" /SD IDOK
  ${EndIf}
SectionEnd

; --- Antes de empezar (va después de las secciones: usa sus nombres) ---

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Kinora necesita Windows de 64 bits." /SD IDOK
    Abort
  ${EndIf}
  SetRegView 64
  ; La 0.2.0 se instalaba para todos los usuarios, en Archivos de programa: hay que sacarla antes.
  ReadRegStr $0 HKLM "Software\Kinora" "InstallDir"
  ${If} $0 != ""
    MessageBox MB_ICONEXCLAMATION "Hay una versión anterior de Kinora instalada para todos los usuarios, en $0.$\r$\n$\r$\nDesinstalala desde Configuración → Aplicaciones (no toca la base ni las películas) y volvé a abrir este instalador." /SD IDOK
    Abort
  ${EndIf}
  ReadRegStr $PreviousVersion HKCU "Software\Kinora" "Version"
  ; Si ffmpeg ya está, la opción de instalarlo viene destildada.
  nsExec::ExecToStack 'cmd /c where ffmpeg'
  Pop $0
  ${If} $0 == 0
    SectionGetFlags ${SecFfmpeg} $1
    IntOp $1 $1 & ${SECTION_OFF}
    SectionSetFlags ${SecFfmpeg} $1
    SectionSetText ${SecFfmpeg} "ffmpeg (ya está instalado)"
  ${EndIf}
FunctionEnd

Function OpenKinora
  Exec '"$INSTDIR\Kinora.exe"'
FunctionEnd

; Al actualizarse desde el Admin (/S /RESTART), Kinora se vuelve a abrir solo al terminar.
Function .onInstSuccess
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/RESTART" $1
  ${IfNot} ${Errors}
    Exec '"$INSTDIR\Kinora.exe"'
  ${EndIf}
FunctionEnd

; --- Desinstalar ---

Function un.onInit
  SetRegView 64
FunctionEnd

Section "Uninstall"
  ExecWait '"$INSTDIR\Kinora.exe" --quit'
  Delete "$INSTDIR\Kinora.exe"
  Delete "$INSTDIR\kinora-server.exe"
  Delete "$INSTDIR\${UNINSTALLER}"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\Kinora.lnk"

  ; La regla del firewall necesita administrador: Windows pide permiso.
  ExecShellWait "runas" "$SYSDIR\netsh.exe" 'advfirewall firewall delete rule name="Kinora"' SW_HIDE
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kinora"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  DeleteRegKey HKCU "Software\Kinora"
SectionEnd
