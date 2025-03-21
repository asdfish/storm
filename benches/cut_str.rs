use {
    criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main},
    std::{borrow::Cow, rc::Rc},
    storm::split_str::SplitStr,
};

pub fn bench_split_str(c: &mut Criterion) {
    const STRING_COUNT: usize = 10;
    const STRING_LEN: usize = 10;

    let mut group = c.benchmark_group("split_str");

    group
        .bench_function("split_str", |b| {
            b.iter_batched(
                || {
                    (0..STRING_COUNT)
                        .map(|_| {
                            (0..STRING_LEN)
                                .map(|_| fastrand::char(..))
                                .collect::<Box<str>>()
                        })
                        .map(Rc::<str>::from)
                        .map(|str| SplitStr::Split {
                            range: 0..str.len(),
                            str,
                        })
                        .collect::<Vec<_>>()
                },
                |strings| {
                    strings.into_iter().for_each(|str| {
                        let (left, right) = str.split_at_checked(black_box(0)).unwrap();
                        black_box(left);
                        black_box(right);
                    })
                },
                BatchSize::SmallInput,
            )
        })
        .bench_function("std", |b| {
            b.iter_batched(
                || {
                    (0..STRING_COUNT)
                        .map(|_| {
                            (0..STRING_LEN)
                                .map(|_| fastrand::char(..))
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                },
                |mut str| {
                    let right = str.split_off(black_box(0));
                    black_box(str);
                    black_box(right);
                },
                BatchSize::SmallInput,
            )
        });
    group.finish();
}

criterion_group!(split_str, bench_split_str);
criterion_main!(split_str);
