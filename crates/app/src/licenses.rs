//! Third-party licenses shown on the About page. The texts are copied into
//! `crates/app/licenses/` and embedded at build time.

pub struct License {
    pub name: &'static str,
    pub license: &'static str,
    pub text: &'static str,
}

pub const LICENSES: &[License] = &[
    License { name: "LAME (libmp3lame)", license: "LGPL-2.0-or-later", text: include_str!("../licenses/lame.txt") },
    License { name: "mp3lame-encoder, mp3lame-sys", license: "LGPL-3.0", text: include_str!("../licenses/lgpl-3.0.txt") },
    License { name: "libvorbis (aoTuV), vorbis_rs", license: "BSD-3-Clause", text: include_str!("../licenses/libvorbis.txt") },
    License { name: "libogg", license: "BSD-3-Clause", text: include_str!("../licenses/libogg.txt") },
    License { name: "Lucide icons", license: "ISC", text: include_str!("../licenses/lucide.txt") },
    License { name: "gpui, gpui-kit", license: "Apache-2.0", text: include_str!("../licenses/gpui-apache-2.0.txt") },
    License { name: "Symphonia", license: "MPL-2.0", text: include_str!("../licenses/mpl-2.0.txt") },
    License { name: "Burn, CubeCL", license: "MIT OR Apache-2.0", text: include_str!("../licenses/burn-mit.txt") },
    License { name: "demucs-rs", license: "Apache-2.0", text: include_str!("../licenses/apache-2.0.txt") },
    License { name: "rubato", license: "MIT OR Apache-2.0", text: include_str!("../licenses/rubato-mit.txt") },
    License { name: "flacenc", license: "Apache-2.0", text: include_str!("../licenses/apache-2.0.txt") },
    License { name: "cpal", license: "Apache-2.0", text: include_str!("../licenses/apache-2.0.txt") },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_license_has_text() {
        assert!(LICENSES.len() >= 10);
        for l in LICENSES {
            assert!(!l.name.is_empty() && !l.license.is_empty());
            assert!(l.text.len() > 200, "{} text looks truncated", l.name);
        }
    }
}
