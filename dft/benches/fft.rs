use std::any::type_name;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use p3_baby_bear::BabyBear;
use p3_dft::{Radix2Bowers, Radix2Dit, Radix2DitParallel, TwoAdicSubgroupDft};
use p3_field::TwoAdicField;
use p3_goldilocks::Goldilocks;
use p3_matrix::dense::RowMajorMatrix;
use p3_mersenne_31::{Mersenne31, Mersenne31Complex, Mersenne31ComplexRadix2Dit, Mersenne31Dft};
use rand::distributions::{Distribution, Standard};
use rand::thread_rng;

fn bench_fft(c: &mut Criterion) {
    // log_sizes correspond to the sizes of DFT we want to benchmark;
    // for the DFT over the quadratic extension "Mersenne31Complex" a
    // fairer comparison is to use half sizes, which is the log minus 1.
    let log_sizes = &[14, 16, 18];
    let log_half_sizes = &[9, 10, 13, 15, 17];

    const BATCH_SIZE: usize = 100;

    fft::<BabyBear, Radix2Dit, BATCH_SIZE>(c, log_sizes);
    fft::<BabyBear, Radix2Bowers, BATCH_SIZE>(c, log_sizes);
    fft::<BabyBear, Radix2DitParallel, BATCH_SIZE>(c, log_sizes);
    fft::<Goldilocks, Radix2Dit, BATCH_SIZE>(c, log_sizes);
    fft::<Goldilocks, Radix2Bowers, BATCH_SIZE>(c, log_sizes);
    fft::<Goldilocks, Radix2DitParallel, BATCH_SIZE>(c, log_sizes);
    fft::<Mersenne31Complex<Mersenne31>, Radix2Dit, BATCH_SIZE>(c, log_half_sizes);
    fft::<Mersenne31Complex<Mersenne31>, Radix2Bowers, BATCH_SIZE>(c, log_half_sizes);
    fft::<Mersenne31Complex<Mersenne31>, Radix2DitParallel, BATCH_SIZE>(c, log_half_sizes);
    fft::<Mersenne31Complex<Mersenne31>, Mersenne31ComplexRadix2Dit, BATCH_SIZE>(c, log_half_sizes);

    m31_fft::<Radix2Dit, BATCH_SIZE>(c, log_sizes);
    m31_fft::<Mersenne31ComplexRadix2Dit, BATCH_SIZE>(c, log_sizes);

    djbfft::<BATCH_SIZE>(c);

    ifft::<Goldilocks, Radix2Dit, BATCH_SIZE>(c);

    coset_lde::<BabyBear, Radix2Bowers, BATCH_SIZE>(c);
    coset_lde::<Goldilocks, Radix2Bowers, BATCH_SIZE>(c);
    coset_lde::<BabyBear, Radix2DitParallel, BATCH_SIZE>(c);
}

fn fft<F, Dft, const BATCH_SIZE: usize>(c: &mut Criterion, log_sizes: &[usize])
where
    F: TwoAdicField,
    Dft: TwoAdicSubgroupDft<F>,
    Standard: Distribution<F>,
{
    let mut group = c.benchmark_group(&format!(
        "fft::<{}, {}, {}>",
        type_name::<F>(),
        type_name::<Dft>(),
        BATCH_SIZE
    ));
    group.sample_size(10);

    let mut rng = thread_rng();
    for n_log in log_sizes {
        let n = 1 << n_log;

        let messages = RowMajorMatrix::rand(&mut rng, n, BATCH_SIZE);

        let dft = Dft::default();
        group.bench_with_input(BenchmarkId::from_parameter(n), &dft, |b, dft| {
            b.iter(|| {
                dft.dft_batch(messages.clone());
            });
        });
    }
}

fn m31_fft<Dft, const BATCH_SIZE: usize>(c: &mut Criterion, log_sizes: &[usize])
where
    Dft: TwoAdicSubgroupDft<Mersenne31Complex<Mersenne31>>,
    Standard: Distribution<Mersenne31>,
{
    let mut group = c.benchmark_group(&format!(
        "m31_fft::<{}, {}>",
        type_name::<Dft>(),
        BATCH_SIZE
    ));
    group.sample_size(10);

    let mut rng = thread_rng();
    for n_log in log_sizes {
        let n = 1 << n_log;

        let messages = RowMajorMatrix::rand(&mut rng, n, BATCH_SIZE);

        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            b.iter(|| {
                Mersenne31Dft::dft_batch::<Dft>(messages.clone());
            });
        });
    }
}

use p3_mersenne_31::split_radix::{forward_fft, Complex};
use rand::Rng;

fn randcomplex(_: usize) -> Complex {
    const P: u32 = (1 << 31) - 1;
    let mut rng = thread_rng();
    let re = rng.gen::<u32>() % P;
    let im = rng.gen::<u32>() % P;
    Complex::new(re.into(), im.into())
}

fn randvec(n: usize) -> Vec<Complex> {
    (0..n)
        .into_iter()
        .map(randcomplex)
        .collect::<Vec<Complex>>()
}

fn djbfft<const BATCH_SIZE: usize>(c: &mut Criterion)
where
    Standard: Distribution<i64>,
{
    let mut group = c.benchmark_group(&format!("djbfft::<{}>", BATCH_SIZE));
    group.sample_size(10);

    let mut v = randvec(16);
    group.bench_function(BenchmarkId::from_parameter(16), |b| {
        b.iter(|| {
            for _ in 0..BATCH_SIZE {
                forward_fft::<16>(&mut v);
            }
        });
    });

    let mut v = randvec(32);
    group.bench_function(BenchmarkId::from_parameter(32), |b| {
        b.iter(|| {
            for _ in 0..BATCH_SIZE {
                forward_fft::<32>(&mut v);
            }
        });
    });

    let mut v = randvec(64);
    group.bench_function(BenchmarkId::from_parameter(64), |b| {
        b.iter(|| {
            for _ in 0..BATCH_SIZE {
                forward_fft::<64>(&mut v);
            }
        });
    });

    let mut v = randvec(512);
    group.bench_function(BenchmarkId::from_parameter(512), |b| {
        b.iter(|| {
            for _ in 0..BATCH_SIZE {
                forward_fft::<512>(&mut v);
            }
        });
    });

    let mut v = randvec(1024);
    group.bench_function(BenchmarkId::from_parameter(1024), |b| {
        b.iter(|| {
            for _ in 0..BATCH_SIZE {
                forward_fft::<1024>(&mut v);
            }
        });
    });

    let mut v = randvec(2048);
    group.bench_function(BenchmarkId::from_parameter(2048), |b| {
        b.iter(|| {
            for _ in 0..BATCH_SIZE {
                forward_fft::<2048>(&mut v);
            }
        });
    });

    let mut v = randvec(4096);
    group.bench_function(BenchmarkId::from_parameter(4096), |b| {
        b.iter(|| {
            for _ in 0..BATCH_SIZE {
                forward_fft::<4096>(&mut v);
            }
        });
    });
}

fn ifft<F, Dft, const BATCH_SIZE: usize>(c: &mut Criterion)
where
    F: TwoAdicField,
    Dft: TwoAdicSubgroupDft<F>,
    Standard: Distribution<F>,
{
    let mut group = c.benchmark_group(&format!(
        "ifft::<{}, {}, {}>",
        type_name::<F>(),
        type_name::<Dft>(),
        BATCH_SIZE
    ));
    group.sample_size(10);

    let mut rng = thread_rng();
    for n_log in [14, 16, 18] {
        let n = 1 << n_log;

        let messages = RowMajorMatrix::rand(&mut rng, n, BATCH_SIZE);

        let dft = Dft::default();
        group.bench_with_input(BenchmarkId::from_parameter(n), &dft, |b, dft| {
            b.iter(|| {
                dft.idft_batch(messages.clone());
            });
        });
    }
}

fn coset_lde<F, Dft, const BATCH_SIZE: usize>(c: &mut Criterion)
where
    F: TwoAdicField,
    Dft: TwoAdicSubgroupDft<F>,
    Standard: Distribution<F>,
{
    let mut group = c.benchmark_group(&format!(
        "coset_lde::<{}, {}, {}>",
        type_name::<F>(),
        type_name::<Dft>(),
        BATCH_SIZE
    ));
    group.sample_size(10);

    let mut rng = thread_rng();
    for n_log in [14, 16, 18] {
        let n = 1 << n_log;

        let messages = RowMajorMatrix::rand(&mut rng, n, BATCH_SIZE);

        let dft = Dft::default();
        group.bench_with_input(BenchmarkId::from_parameter(n), &dft, |b, dft| {
            b.iter(|| {
                dft.coset_lde_batch(messages.clone(), 1, F::generator());
            });
        });
    }
}

criterion_group!(benches, bench_fft);
criterion_main!(benches);
