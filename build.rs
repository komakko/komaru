#[cfg(windows)]
fn main() {
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/icon/komaru.ico");
    resource.set("ProductName", "Komaru");
    resource.set("FileDescription", "Komaru");
    if let Err(error) = resource.compile() {
        println!("cargo::error=windows resource failed: {error}");
    }
}

#[cfg(not(windows))]
fn main() {}
