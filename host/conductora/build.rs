//use dotenvy::from_path_iter;
//use std::path::Path;

fn main() {
    // Load .env at compile time
    //let env_path = Path::new(".env");
    //if env_path.exists() {
    //    for item in from_path_iter(env_path).expect("Failed to parse .env") {
   //         let (key, value) = item.expect("Invalid .env entry");
    //        println!("cargo:rustc-env={}={}", key, value);
   //     }
   // }

    // You can also set rerun-if-changed
    //println!("cargo:rerun-if-changed=.env");
    tauri_build::build()
}
