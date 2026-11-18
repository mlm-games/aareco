fn main() {
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    {
        if let Err(err) = aareco::run_desktop() {
            eprintln!("Aareco failed to start: {err}");
        }
    }
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn main() {}
