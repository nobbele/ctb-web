use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/");

    Command::new("tailwindcss")
        .env("NODE_ENV", "production")
        .args([
            "-c",
            "./tailwind.config.js",
            "-o",
            "./tailwind.css",
            "--minify",
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}
