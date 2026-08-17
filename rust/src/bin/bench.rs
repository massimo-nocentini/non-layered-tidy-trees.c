//! Wall clock over one large tree, the workload `simd-plan.md` scopes for.
//!
//! Prints the same table as `test/bench.c`, over the very same trees -- `treegen::bench_tree`
//! and `tg_bench_tree` draw from the same generator in the same order -- so the rows can be
//! concatenated and read as one comparison. The checksum is the XOR of the bit patterns of
//! every coordinate, which is exact and order independent: every row of the table has to
//! carry the same checksum for a given `(n, vert)`, whatever the implementation.
//!
//! ```sh
//! cargo run --release --bin bench                      # the recursive and flat paths
//! cargo run --release --features simd --bin bench      # and the explicitly vectorized one
//! make -C ../test bench                                # the C row of the same table
//! ```

use std::env;
use std::time::Instant;

use non_layered_tidy_trees::flat::Engine;
use non_layered_tidy_trees::treegen;
use non_layered_tidy_trees::{flat, layout, layout_api, Arena, LayoutInput};

const SEED: u64 = 20240607;
const MAXKIDS: usize = 4;

/// One implementation under test.
///
/// The engine rows keep their mirror between calls, which is what the `layout_flat` rows
/// do not: for a large tree, allocating and faulting in a hundred megabytes of arrays is
/// more work than the layout itself, and the two rows together say how much.
type Runner = Box<dyn FnMut(&mut Arena, &LayoutInput) + Send>;

fn impls() -> Vec<(&'static str, Runner)> {
    let mut v: Vec<(&'static str, Runner)> = vec![
        ("rust-rec", Box::new(layout)),
        ("rust-flat", Box::new(flat::layout_flat)),
    ];

    let mut engine = Engine::new();
    v.push((
        "rust-flat-r",
        Box::new(move |a: &mut Arena, i: &LayoutInput| engine.layout(a, i)),
    ));

    #[cfg(feature = "simd")]
    {
        v.push(("rust-simd", Box::new(flat::layout_flat_simd)));

        let mut engine = Engine::with_simd();
        v.push((
            "rust-simd-r",
            Box::new(move |a: &mut Arena, i: &LayoutInput| engine.layout(a, i)),
        ));
    }

    v
}

fn header() {
    #[cfg(feature = "simd")]
    println!(
        "# kernels: {}",
        if Engine::with_simd().kernels().is_simd() {
            "avx2"
        } else {
            "scalar (no AVX2 on this CPU)"
        }
    );
    println!(
        "# {:<16} {:>9} {:>5} {:>11} {:>11} {:>16}",
        "impl", "n", "vert", "ns/node", "ms", "checksum"
    );
}

fn row(name: &str, n: usize, vert: bool, ms: f64, ck: u64) {
    println!(
        "{:<18} {:>9} {:>5} {:>11.2} {:>11.3} {:016x}",
        name,
        n,
        vert as u8,
        ms * 1e6 / n as f64,
        ms,
        ck
    );
}

/// The best of `reps` batches, each of enough iterations to outrun the clock.
///
/// The first layout of a freshly built tree also first-touches its pages, so it is a
/// warm-up rather than a measurement; `layout` may be applied repeatedly to one tree,
/// which is what makes this legitimate.
fn best_ms(f: &mut Runner, arena: &mut Arena, input: &LayoutInput, reps: usize) -> f64 {
    let t0 = Instant::now();
    f(arena, input);
    let warm = t0.elapsed().as_secs_f64() * 1e3;

    let inner = if warm > 0.0 {
        ((20.0 / warm) as usize).clamp(1, 1000)
    } else {
        1000
    };

    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let s = Instant::now();
        for _ in 0..inner {
            f(arena, input);
        }
        let dt = s.elapsed().as_secs_f64() * 1e3 / inner as f64;
        best = best.min(dt);
    }

    best
}

/// `layout_api`, which takes its tree as arrays and gives the coordinates back in the same
/// ones -- so every iteration has to restore the inputs first. That restore is timed too,
/// for every implementation alike, since it is what a caller laying out repeatedly would do.
type ApiRunner = Box<dyn FnMut(usize, &mut [f64], &[f64], &[usize], bool)>;

fn api_impls() -> Vec<(&'static str, ApiRunner)> {
    let mut v: Vec<(&'static str, ApiRunner)> = vec![
        (
            "rust-api",
            Box::new(|n, wh: &mut [f64], whg: &[f64], c: &[usize], vert| {
                layout_api(n, wh, whg, c, 1, vert, false, 0.0, 0.0)
            }),
        ),
        (
            "rust-api-flat",
            Box::new(|n, wh: &mut [f64], whg: &[f64], c: &[usize], vert| {
                flat::layout_api_flat(n, wh, whg, c, 1, vert, false, 0.0, 0.0)
            }),
        ),
    ];

    let mut engine = Engine::new();
    v.push((
        "rust-api-flat-r",
        Box::new(move |n, wh: &mut [f64], whg: &[f64], c: &[usize], vert| {
            engine.layout_api(n, wh, whg, c, 1, vert, false, 0.0, 0.0)
        }),
    ));

    #[cfg(feature = "simd")]
    {
        let mut engine = Engine::with_simd();
        v.push((
            "rust-api-simd-r",
            Box::new(move |n, wh: &mut [f64], whg: &[f64], c: &[usize], vert| {
                engine.layout_api(n, wh, whg, c, 1, vert, false, 0.0, 0.0)
            }),
        ));
    }

    v
}

/// The arrays one `layout_api` call needs, and the pristine copy to restore between them.
struct ApiCase {
    n: usize,
    wh: Vec<f64>,
    pristine: Vec<f64>,
    whg: Vec<f64>,
    children: Vec<usize>,
    vert: bool,
}

impl ApiCase {
    fn new(n: usize, vert: bool) -> ApiCase {
        let (wh, whg, children) = treegen::bench_arrays(n, MAXKIDS, SEED);
        ApiCase {
            n,
            pristine: wh.clone(),
            wh,
            whg,
            children,
            vert,
        }
    }

    fn checksum(&self) -> u64 {
        self.wh[..2 * self.n]
            .iter()
            .fold(0u64, |c, v| c ^ v.to_bits())
    }
}

fn best_api_ms(f: &mut ApiRunner, case: &mut ApiCase, reps: usize) -> f64 {
    let ApiCase {
        n,
        wh,
        pristine,
        whg,
        children,
        vert,
    } = case;
    let (n, vert) = (*n, *vert);
    wh.copy_from_slice(pristine);
    let t0 = Instant::now();
    f(n, wh, whg, children, vert);
    let warm = t0.elapsed().as_secs_f64() * 1e3;

    let inner = if warm > 0.0 {
        ((20.0 / warm) as usize).clamp(1, 1000)
    } else {
        1000
    };

    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let s = Instant::now();
        for _ in 0..inner {
            wh.copy_from_slice(pristine);
            f(n, wh, whg, children, vert);
        }
        best = best.min(s.elapsed().as_secs_f64() * 1e3 / inner as f64);
    }

    best
}

fn checksum(arena: &Arena) -> u64 {
    arena
        .iter()
        .fold(0u64, |ck, n| ck ^ n.x.to_bits() ^ n.y.to_bits())
}

/// Where the time goes inside one flat layout, which is what `simd-plan.md` asks for.
fn phases(reps: usize) {
    println!("# phase breakdown of one flat layout, best of {reps}, us");
    println!(
        "# {:<10} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}",
        "impl", "n", "build", "setup", "first", "second", "third", "write"
    );

    #[allow(unused_mut)]
    let mut engines: Vec<(&str, Engine)> = vec![("flat", Engine::new())];
    #[cfg(feature = "simd")]
    engines.push(("simd", Engine::with_simd()));

    for (name, engine) in engines.iter_mut() {
        for n in [10_000usize, 1_000_000] {
            let mut arena = Arena::with_capacity(n);
            let root = treegen::bench_tree(&mut arena, n, MAXKIDS, SEED);
            let input = LayoutInput::new(root);

            engine.layout(&mut arena, &input); // warm up the mirror

            let mut best = engine.profile(&mut arena, &input);
            for _ in 1..reps {
                let p = engine.profile(&mut arena, &input);
                if p.total() < best.total() {
                    best = p;
                }
            }

            print!("{name:<12} {n:>9}");
            for (_, d) in best.iter() {
                print!(" {:>9.1}", d.as_secs_f64() * 1e6);
            }
            println!();
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let reps = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .and_then(|a| a.parse().ok())
        .unwrap_or(5);

    let deep = args.iter().position(|a| a == "--deep").map(|i| {
        args.get(i + 1)
            .and_then(|d| d.parse().ok())
            .unwrap_or(10000)
    });

    if args.iter().any(|a| a == "--phases") {
        phases(reps);
        return;
    }

    header();

    for n in [1_000usize, 10_000, 100_000, 1_000_000] {
        for vert in [true, false] {
            for (name, f) in impls().iter_mut() {
                let mut arena = Arena::with_capacity(n);
                let root = treegen::bench_tree(&mut arena, n, MAXKIDS, SEED);
                let input = LayoutInput {
                    root,
                    vertically: vert,
                    centeredxy: false,
                    x: 0.0,
                    y: 0.0,
                };

                let ms = best_ms(f, &mut arena, &input, reps);
                row(name, n, vert, ms, checksum(&arena));
            }
        }
    }

    for n in [1_000usize, 10_000, 100_000, 1_000_000] {
        for vert in [true, false] {
            for (name, f) in api_impls().iter_mut() {
                let mut case = ApiCase::new(n, vert);
                let ms = best_api_ms(f, &mut case, reps);
                row(name, n, vert, ms, case.checksum());
            }
        }
    }

    if let Some(depth) = deep {
        // The recursive path is bounded by the stack, so it runs on a thread with a big
        // one; the flat path recurses nowhere and does not need it.
        for (name, mut f) in impls() {
            let worker = std::thread::Builder::new()
                .stack_size(512 << 20)
                .spawn(move || {
                    let mut arena = Arena::with_capacity(depth);
                    let root = treegen::chain(&mut arena, 1, depth, 10.0, 10.0);
                    let input = LayoutInput::new(root);
                    let ms = best_ms(&mut f, &mut arena, &input, reps);
                    (ms, checksum(&arena))
                })
                .expect("spawning the deep-walk thread");

            let (ms, ck) = worker.join().expect("laying out a deep chain");
            row(&format!("{name}-chain"), depth, true, ms, ck);
        }
    }
}
