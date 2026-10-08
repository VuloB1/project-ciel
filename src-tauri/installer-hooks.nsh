; Icono propio para los archivos de Project Ciel (.aivtprofile y
; .aivtbackup). Tauri v2 no tiene un campo de icono por asociación de
; archivo, así que se escribe a mano la clave DefaultIcon de cada clase
; después de que el instalador registre las asociaciones.
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "Software\Classes\ProjectCielProfile\DefaultIcon" "" "$INSTDIR\file-icon.ico"
  WriteRegStr SHCTX "Software\Classes\ProjectCielBackup\DefaultIcon" "" "$INSTDIR\file-icon.ico"
  ; Avisa al Explorador para que refresque los iconos sin reiniciar sesión.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
