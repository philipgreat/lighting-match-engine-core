use std::process::Command;

fn main() {


    

    let output = Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%d %H:%M:%S UTC")
        .output()
        .unwrap();

    let build_time = String::from_utf8(output.stdout).unwrap();
    let build_time = build_time.trim();

    println!("cargo:rustc-env=BUILD_TIME={}", build_time);

    let hostname = Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_HOSTNAME={}", hostname);


    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .or_else(|_| {
            Command::new("whoami")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .ok_or(std::env::VarError::NotPresent)
        })
        .unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_USER={}", user);
    
}
