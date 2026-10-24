// El ícono y los datos del programa dentro de Kinora.exe (los que muestran el Explorador y
// "Agregar o quitar programas"). Solo al compilar para Windows.

use std::path::PathBuf;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let crate_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = crate_dir.join("../assets/kinora.ico");
    println!("cargo:rerun-if-changed={}", icon.display());

    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let numbers: Vec<&str> = version.split('.').collect();
    let comma = format!("{},{},{},0", numbers[0], numbers[1], numbers[2]);
    // El manifiesto: los controles comunes de Windows en su versión moderna (los menús los usan),
    // nitidez en pantallas de alta densidad y Windows 10 u 11.
    let manifest_file = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("kinora.manifest");
    std::fs::write(
        &manifest_file,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="Kinora.Tray" version="1.0.0.0" processorArchitecture="amd64"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
"#,
    )
    .unwrap();

    let rc = format!(
        r#"1 ICON "{icon}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {comma}
PRODUCTVERSION {comma}
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040A04B0"
    BEGIN
      VALUE "CompanyName", "Kinora"
      VALUE "FileDescription", "Kinora"
      VALUE "FileVersion", "{version}"
      VALUE "ProductName", "Kinora"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x040A, 1200
  END
END
"#,
        icon = icon.display().to_string().replace('\\', "/"),
        manifest = manifest_file.display().to_string().replace('\\', "/"),
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("kinora.rc");
    std::fs::write(&out, rc).unwrap();
    embed_resource::compile(&out, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
