mod controller_worker;
mod gtk_host;
mod power;
mod users;

fn main() -> gtk4::glib::ExitCode {
    gtk_host::run()
}
