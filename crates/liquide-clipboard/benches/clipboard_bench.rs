use criterion::{Criterion, black_box, criterion_group, criterion_main};

use liquide_clipboard::format::ClipboardFormat;
use liquide_clipboard::offer::ClipboardOffer;
use liquide_clipboard::store::ClipboardStore;
use liquide_clipboard::transfer::ClipboardTransfer;

fn bench_store_set_get_1kb(c: &mut Criterion) {
    c.bench_function("store_set_get_1kb", |b| {
        let data = vec![0x42u8; 1024];
        b.iter(|| {
            let mut store = ClipboardStore::new(1024 * 1024);
            store
                .set(ClipboardFormat::PlainText, black_box(data.clone()), 1, 0)
                .unwrap();
            let _ = black_box(store.get(&ClipboardFormat::PlainText));
        })
    });
}

fn bench_format_from_mime_lookup(c: &mut Criterion) {
    c.bench_function("format_from_mime_lookup", |b| {
        let mimes = [
            "text/plain;charset=utf-8",
            "text/html",
            "image/png",
            "image/jpeg",
            "text/plain",
            "text/uri-list",
            "image/svg+xml",
            "text/richtext",
            "application/unknown",
            "text/plain;charset=utf-8",
        ];
        b.iter(|| {
            for &mime in &mimes {
                let _ = black_box(ClipboardFormat::from_mime(black_box(mime)));
            }
        })
    });
}

fn bench_transfer_receive_1mb_chunked(c: &mut Criterion) {
    c.bench_function("transfer_receive_1mb_chunked", |b| {
        let chunk = vec![0xABu8; 64 * 1024]; // 64 KB chunks
        b.iter(|| {
            let mut t = ClipboardTransfer::new(2 * 1024 * 1024);
            let offer = ClipboardOffer::new(1, vec![ClipboardFormat::PlainText], 0, 1);
            t.begin_offer(offer);
            t.request_format(ClipboardFormat::PlainText).unwrap();
            for _ in 0..16 {
                // 16 * 64KB = 1MB
                t.receive_chunk(black_box(&chunk)).unwrap();
            }
            let _ = black_box(t.complete().unwrap());
        })
    });
}

criterion_group!(
    benches,
    bench_store_set_get_1kb,
    bench_format_from_mime_lookup,
    bench_transfer_receive_1mb_chunked,
);
criterion_main!(benches);
