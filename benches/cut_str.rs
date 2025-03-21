use {
    criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main},
    std::{borrow::Cow, rc::Rc},
    storm::cut_str::CutStr,
};

pub fn bench_cut_str(c: &mut Criterion) {
    const STRING_COUNT: usize = 10;
    const STRING_LEN: usize = 10;

    let mut group = c.benchmark_group("cut_str");

    group
        .bench_function("cut_str", |b| {
            b.iter_batched(
                || {
                    (0..STRING_COUNT)
                        .map(|_| {
                            (0..STRING_LEN)
                                .map(|_| fastrand::char(..))
                                .collect::<String>()
                        })
                        .map(|str| CutStr::Cut {
                            head: 0,
                            str,
                        })
                        .collect::<Vec<_>>()
                },
                |strings| {
                    strings.into_iter().for_each(|str| {
                        let str = str.cut_checked(black_box(0)).unwrap();
                        black_box(str);
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

criterion_group!(cut_str, bench_cut_str);
criterion_main!(cut_str);
