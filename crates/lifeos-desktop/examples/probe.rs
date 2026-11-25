//! `cargo run -p lifeos-desktop --example probe -- <op> [input json]`: runs one desktop tool here and prints it.
fn main() {
    #[cfg(target_os = "macos")]
    {
        let args: Vec<String> = std::env::args().collect();
        let op = args.get(1).map_or("desktop_look", String::as_str);
        let input = args.get(2).map_or("{}", String::as_str);
        println!("{:?} screen {:?}", lifeos_desktop::Permissions::check(false), lifeos_desktop::screen_size());
        let mut d = lifeos_desktop::Desktop::default();
        let t = std::time::Instant::now();
        match d.run(op, input) {
            Ok(a) => println!("{}\n[{} ms, jpeg {} bytes]", a.text, t.elapsed().as_millis(), a.jpeg.map_or(0, |j| j.len())),
            Err(e) => println!("error: {e:#}"),
        }
    }
}
