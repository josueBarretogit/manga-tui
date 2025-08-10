use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use manga_tui::SanitizedFilename;

fn sanitize_filename(c: &mut Criterion) {
    let example = "some / title :";

    c.bench_with_input(BenchmarkId::new("sanitized string parsing", example), &example, |b, &ex| {
        b.iter(|| SanitizedFilename::new(ex));
    });
}

criterion_group!(benches, sanitize_filename);
criterion_main!(benches);
