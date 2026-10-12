fn main() {
    tauri_build::build();

    #[cfg(windows)]
    {
        use std::path::Path;
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let manifest_path = Path::new(&out_dir).join("comctl6.manifest");
        let rc_path = Path::new(&out_dir).join("comctl6.rc");
        let res_path = Path::new(&out_dir).join("comctl6.o");

        let manifest = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>"#;

        let _ = std::fs::write(&manifest_path, manifest);
        let rc_content = format!(r#"1 24 "{}""#, manifest_path.to_str().unwrap().replace('\\', "/"));
        let _ = std::fs::write(&rc_path, rc_content);

        if let Ok(status) = std::process::Command::new("windres")
            .arg("-i")
            .arg(&rc_path)
            .arg("-o")
            .arg(&res_path)
            .status()
        {
            if status.success() {
                println!("cargo:rustc-link-arg={}", res_path.to_str().unwrap().replace('\\', "/"));
            }
        }
    }
}

