// Recompila cuando cambian las migraciones, porque `sqlx::migrate!` las incluye en el binario.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
