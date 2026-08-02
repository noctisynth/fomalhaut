mod config;
mod controller_worker;
mod gtk_host;

fn main() -> gtk4::glib::ExitCode {
    gtk_host::run()
}
