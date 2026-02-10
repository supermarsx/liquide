use criterion::{criterion_group, criterion_main, Criterion};

use liquide_interop::desktop_entry::DesktopEntry;
use liquide_interop::mime::{MimeAssociation, MimeDatabase, MimeSource, MimeType};

const SAMPLE_DESKTOP: &str = "\
[Desktop Entry]
Type=Application
Name=TestApp
GenericName=Test Application
Comment=A test application
Icon=test-app
Exec=/usr/bin/testapp %u
Categories=Development;IDE;
MimeType=text/plain;text/x-csrc;
Keywords=editor;code;
";

fn bench_parse_1000_desktop_entries(c: &mut Criterion) {
    c.bench_function("parse_1000_desktop_entries", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                let _ = DesktopEntry::parse(SAMPLE_DESKTOP).unwrap();
            }
        });
    });
}

fn bench_mime_lookup_10000_queries(c: &mut Criterion) {
    let mut db = MimeDatabase::new();
    for i in 0..100 {
        let mt = MimeType {
            type_: "application".to_string(),
            subtype: format!("x-type-{i}"),
        };
        db.add_association(MimeAssociation {
            mime_type: mt,
            desktop_entry_id: format!("app-{i}.desktop"),
            source: MimeSource::System,
        });
    }

    let query = MimeType::parse("application/x-type-50").unwrap();

    c.bench_function("mime_lookup_10000_queries", |b| {
        b.iter(|| {
            for _ in 0..10_000 {
                let _ = db.lookup(&query);
            }
        });
    });
}

criterion_group!(
    benches,
    bench_parse_1000_desktop_entries,
    bench_mime_lookup_10000_queries
);
criterion_main!(benches);
