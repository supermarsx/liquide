//! Sort utilities for directory listings.

use crate::config::SortField;
use crate::entry::FileEntry;

/// Sort file entries by the given field and direction.
pub fn sort_entries(entries: &mut [FileEntry], field: SortField, ascending: bool) {
    entries.sort_by(|a, b| {
        // Directories always come before files.
        let dir_cmp = b.is_dir().cmp(&a.is_dir());
        if dir_cmp != std::cmp::Ordering::Equal {
            return dir_cmp;
        }

        let cmp = match field {
            SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortField::Size => a.size.cmp(&b.size),
            SortField::Modified => a.modified.cmp(&b.modified),
            SortField::Type => a.extension.to_lowercase().cmp(&b.extension.to_lowercase()),
        };

        if ascending { cmp } else { cmp.reverse() }
    });
}

/// Sort entries with a natural sort for names (handles numbers in names).
pub fn sort_natural(entries: &mut [FileEntry], ascending: bool) {
    entries.sort_by(|a, b| {
        let dir_cmp = b.is_dir().cmp(&a.is_dir());
        if dir_cmp != std::cmp::Ordering::Equal {
            return dir_cmp;
        }
        let cmp = natural_cmp(&a.name, &b.name);
        if ascending { cmp } else { cmp.reverse() }
    });
}

/// Natural string comparison (foo2 < foo10).
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();

    loop {
        match (ai.peek(), bi.peek()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(ac), Some(bc)) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    let an = collect_number(&mut ai);
                    let bn = collect_number(&mut bi);
                    match an.cmp(&bn) {
                        std::cmp::Ordering::Equal => continue,
                        other => return other,
                    }
                }
                let ac = ac.to_lowercase().next().unwrap_or(*ac);
                let bc = bc.to_lowercase().next().unwrap_or(*bc);
                match ac.cmp(&bc) {
                    std::cmp::Ordering::Equal => {
                        ai.next();
                        bi.next();
                    }
                    other => return other,
                }
            }
        }
    }
}

fn collect_number(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> u64 {
    let mut n: u64 = 0;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            n = n.saturating_mul(10).saturating_add(c as u64 - '0' as u64);
            chars.next();
        } else {
            break;
        }
    }
    n
}
