use std::{env, fs, path::PathBuf};

fn strip_legacy_command_attrs(file_name: &str, function_names: &[&str]) {
    let source_path = PathBuf::from("src").join(file_name);
    println!("cargo:rerun-if-changed={}", source_path.display());
    let mut source = fs::read_to_string(&source_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", source_path.display()));

    for function_name in function_names {
        let replacement = format!("pub fn {function_name}");
        let lf = format!("#[tauri::command]\npub fn {function_name}");
        let crlf = format!("#[tauri::command]\r\npub fn {function_name}");
        if source.contains(&lf) {
            source = source.replacen(&lf, &replacement, 1);
        } else if source.contains(&crlf) {
            source = source.replacen(&crlf, &replacement, 1);
        } else {
            panic!(
                "failed to de-expose legacy command wrapper {function_name} in {}",
                source_path.display()
            );
        }
    }

    let out_path = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join(file_name);
    fs::write(&out_path, source)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", out_path.display()));
}

fn main() {
    strip_legacy_command_attrs("commands.rs", &["close_waiting_for"]);
    strip_legacy_command_attrs(
        "negotiation.rs",
        &["change_insurance_claim_status", "get_negotiation_snapshot"],
    );
    tauri_build::build()
}
