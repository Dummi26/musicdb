#![cfg(test)]
use std::{assert_eq, path::PathBuf};

use crate::load::ToFromBytes;

#[test]
fn string() {
    for v in ["dskjh2d89dnas2d90", "aosu 89d 89a 89", "a/b/c/12"] {
        let v = v.to_owned();
        assert_eq!(v, String::from_bytes(&mut &v.to_bytes_vec()[..]).unwrap());
        let v = PathBuf::from(v);
        assert_eq!(v, PathBuf::from_bytes(&mut &v.to_bytes_vec()[..]).unwrap());
    }
}

#[test]
fn vec() {
    for v in [vec!["asdad".to_owned(), "dsnakf".to_owned()], vec![]] {
        assert_eq!(
            v,
            Vec::<String>::from_bytes(&mut &v.to_bytes_vec()[..]).unwrap()
        )
    }
}

#[test]
fn option() {
    for v in [None, Some("value".to_owned())] {
        assert_eq!(
            v,
            Option::<String>::from_bytes(&mut &v.to_bytes_vec()[..]).unwrap()
        )
    }
}
