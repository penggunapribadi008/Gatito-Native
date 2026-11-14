fn main() {
    // Repo GitHub untuk "Cek update" diambil saat kompilasi (CI mengisi GITHUB_REPOSITORY otomatis)
    println!("cargo:rerun-if-env-changed=GITHUB_REPOSITORY");
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Gatito Stream Tool");
        res.set("FileDescription", "Gatito: Stream Tool");
        let _ = res.compile();
    }
}
