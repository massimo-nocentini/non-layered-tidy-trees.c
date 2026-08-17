//! The differential oracle of `test/overlap.c --dump`, and its comparison half.
//!
//! The same corpus, the same trees, the same order; only the printing of the doubles
//! differs, because C's `%.17g` and Rust's shortest round-trip formatting spell the same
//! value differently. `--check` therefore parses a dump instead of diffing it, which
//! compares the values themselves — and since `%.17g` round-trips, the comparison is
//! exact, not approximate.
//!
//! ```sh
//! make -C ../test overlap && ../test/overlap --dump 10 > golden.txt   # the C coordinates
//! cargo run --release --bin dump -- --check golden.txt 10             # the Rust ones
//! ```

use std::env;
use std::fs;
use std::process::ExitCode;

use non_layered_tidy_trees::treegen::{self, Regime, REGIMES};
use non_layered_tidy_trees::{flat, layout, Arena, LayoutInput, NodeId};

/// Which implementation to dump or check; they all have to agree, bit for bit.
type Impl = fn(&mut Arena, &LayoutInput);

fn pick(args: &[String]) -> (Impl, &'static str) {
    if args.iter().any(|a| a == "--simd") {
        #[cfg(feature = "simd")]
        return (flat::layout_flat_simd, "flat+simd");
        #[cfg(not(feature = "simd"))]
        {
            eprintln!("built without `--features simd`; falling back to the scalar kernels");
            return (flat::layout_flat, "flat");
        }
    }
    if args.iter().any(|a| a == "--flat") {
        return (flat::layout_flat, "flat");
    }
    (layout, "recursive")
}

/// One tree of the corpus, in the order `overlap.c` sweeps them.
struct Shape {
    regime: Regime,
    maxdepth: usize,
    maxkids: usize,
    vert: bool,
    cent: bool,
    k: usize,
}

impl Shape {
    fn seed(&self) -> u64 {
        1000003 * self.k as u64
            + 7 * (self.regime.index() * 100 + self.maxdepth * 10 + self.maxkids) as u64
    }

    fn header(&self, n: usize) -> String {
        format!(
            "# regime={} depth={} kids={} vert={} cent={} k={} n={}",
            self.regime.index(),
            self.maxdepth,
            self.maxkids,
            self.vert as u8,
            self.cent as u8,
            self.k,
            n
        )
    }
}

fn corpus(trials: usize) -> impl Iterator<Item = Shape> {
    REGIMES.into_iter().flat_map(move |regime| {
        (1..=6).flat_map(move |maxdepth| {
            (2..=5).flat_map(move |maxkids| {
                [false, true].into_iter().flat_map(move |vert| {
                    [false, true].into_iter().flat_map(move |cent| {
                        (0..trials).map(move |k| Shape {
                            regime,
                            maxdepth,
                            maxkids,
                            vert,
                            cent,
                            k,
                        })
                    })
                })
            })
        })
    })
}

fn lay_out(shape: &Shape, run: Impl) -> (Arena, Vec<NodeId>) {
    let (mut arena, root) =
        treegen::build(shape.seed(), shape.maxdepth, shape.maxkids, shape.regime);

    let input = LayoutInput {
        root,
        vertically: shape.vert,
        centeredxy: shape.cent,
        x: 0.0,
        y: 0.0,
    };
    run(&mut arena, &input);

    let nodes = treegen::collect(&arena, root);
    (arena, nodes)
}

fn dump(trials: usize, run: Impl) {
    let mut out = String::new();
    for shape in corpus(trials) {
        let (arena, nodes) = lay_out(&shape, run);
        out.push_str(&shape.header(nodes.len()));
        out.push('\n');
        for &id in &nodes {
            let n = &arena[id];
            out.push_str(&format!("{} {:?} {:?}\n", n.idx, n.x, n.y));
        }
        if out.len() > 1 << 20 {
            print!("{out}");
            out.clear();
        }
    }
    print!("{out}");
}

/// Compares the corpus against a dump, value by value; the exit code is the verdict.
fn check(path: &str, trials: usize, run: Impl, name: &str) -> ExitCode {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("FAIL  cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let mut mismatches = 0usize;
    let mut compared = 0usize;
    let mut trees = 0usize;

    for shape in corpus(trials) {
        let (arena, nodes) = lay_out(&shape, run);
        trees += 1;

        let header = match lines.next() {
            Some(l) => l,
            None => {
                eprintln!("FAIL  {path} ends after {trees} trees, the corpus does not");
                return ExitCode::FAILURE;
            }
        };
        if header != shape.header(nodes.len()) {
            eprintln!(
                "FAIL  shape mismatch\n  expected {}\n  found    {header}",
                shape.header(nodes.len())
            );
            return ExitCode::FAILURE;
        }

        for &id in &nodes {
            let n = &arena[id];
            let line = lines.next().unwrap_or("");
            let mut it = line.split_whitespace();
            let idx: usize = it.next().and_then(|f| f.parse().ok()).unwrap_or(0);
            let x: f64 = it.next().and_then(|f| f.parse().ok()).unwrap_or(f64::NAN);
            let y: f64 = it.next().and_then(|f| f.parse().ok()).unwrap_or(f64::NAN);

            compared += 2;

            // Bit-exact: the two implementations do the same operations in the same order.
            if idx != n.idx || x.to_bits() != n.x.to_bits() || y.to_bits() != n.y.to_bits() {
                mismatches += 1;
                if mismatches <= 10 {
                    eprintln!(
                        "  {}: node {} is [{:?}, {:?}] here and [{x:?}, {y:?}] there",
                        shape.header(nodes.len()),
                        n.idx,
                        n.x,
                        n.y
                    );
                }
            }
        }
    }

    if mismatches == 0 {
        eprintln!("PASS  {compared} coordinates over {trees} trees, {name} identical to {path}");
        ExitCode::SUCCESS
    } else {
        eprintln!("FAIL  {name}: {mismatches} nodes differ from {path} ({compared} coordinates over {trees} trees)");
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    let all: Vec<String> = env::args().skip(1).collect();
    let (run, name) = pick(&all);

    let args: Vec<String> = all
        .into_iter()
        .filter(|a| !matches!(a.as_str(), "--flat" | "--simd"))
        .collect();

    match args.first().map(String::as_str) {
        Some("--check") => {
            let path = match args.get(1) {
                Some(p) => p,
                None => {
                    eprintln!("usage: dump --check <file> [trials]");
                    return ExitCode::FAILURE;
                }
            };
            let trials = args.get(2).and_then(|t| t.parse().ok()).unwrap_or(300);
            check(path, trials, run, name)
        }
        Some("--help") | Some("-h") => {
            println!(
                "usage: dump [trials]                  coordinates on stdout, 300 trials per shape"
            );
            println!("       dump --check <file> [trials]   compare against a dump of the C build");
            println!(
                "       --flat / --simd                use the sweeps instead of the recursion"
            );
            ExitCode::SUCCESS
        }
        other => {
            let trials = other.and_then(|t| t.parse().ok()).unwrap_or(300);
            dump(trials, run);
            ExitCode::SUCCESS
        }
    }
}
