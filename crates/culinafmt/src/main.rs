//! Smoke CLI: parse stdin (or files given as arguments), print canonical
//! text to stdout. The gofmt flag surface (-l, -d, in-place) lands with the
//! formatter phase; this exists to exercise the parser by hand.

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let inputs: Vec<(String, String)> = if args.is_empty() {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .expect("read stdin");
        vec![("<stdin>".to_string(), buf)]
    } else {
        args.iter()
            .map(|p| {
                let text = std::fs::read_to_string(p).unwrap_or_else(|e| {
                    eprintln!("culinafmt: {p}: {e}");
                    std::process::exit(1);
                });
                (p.clone(), text)
            })
            .collect()
    };

    for (name, text) in inputs {
        match culina_core::parse::parse(&text) {
            Ok(doc) => print!("{}", culina_core::print::print(&doc)),
            Err(e) => {
                eprintln!("culinafmt: {name}: {e}");
                std::process::exit(1);
            }
        }
    }
}
