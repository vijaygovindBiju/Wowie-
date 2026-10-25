# Third-Party Notices and License Inventory

Wowie incorporates open-source software and digital assets under various permissive licenses.
This document provides full attribution, license classifications, and repository references for all direct and transitive dependencies and bundled assets.

---

## 1. Summary of Licenses

All third-party dependencies used in Wowie are licensed under permissive open-source licenses compatible with the project's dual license (`MIT OR Apache-2.0`).

| License Type | Crate Count | Compatibility Assessment |
|---|---|---|
| **MIT OR Apache-2.0** / **Apache-2.0 OR MIT** | 156 | Fully compatible dual-license |
| **MIT** | 67 | Permissive; full attribution retained |
| **Unicode-3.0** | 18 | Permissive; character data / collation |
| **Zlib / Zlib OR Apache-2.0 OR MIT** | 15 | Permissive compression library license |
| **BSD (2-Clause / 3-Clause)** | 6 | Permissive; attribution retained |
| **Apache-2.0** (standalone) | 3 | Permissive patent & copyright grant |
| **ISC** | 3 | Permissive BSD-equivalent |
| **Unlicense / 0BSD** | 7 | Public domain dedications |
| **CDLA-Permissive-2.0** | 1 | Permissive data/model license |

Zero viral copyleft (GPL, AGPL) or non-commercial licenses are included in the runtime binaries.

---

## 2. Direct Workspace Dependencies

The following table details all direct dependencies declared across the Wowie Rust workspace:

| Crate | Version | License | Upstream Repository / Homepage | Purpose in Wowie |
|---|---|---|---|---|
| `tokio` | 1.40+ | MIT | https://github.com/tokio-rs/tokio | Asynchronous runtime, timers, process I/O |
| `serde` | 1.0 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde | Serialization framework |
| `serde_json` | 1.0 | MIT OR Apache-2.0 | https://github.com/serde-rs/json | JSON encoding for IPC, DB, and Native Messaging |
| `chrono` | 0.4 | MIT OR Apache-2.0 | https://github.com/chronotope/chrono | Date, UTC timestamps, focus duration math |
| `uuid` | 1.10 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid | RFC 4122 UUIDv4 generation for entities |
| `thiserror` | 2.0 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror | Type-safe error derive implementations |
| `rusqlite` | 0.32 | MIT | https://github.com/rusqlite/rusqlite | Local SQLite storage and migration transactions |
| `sha2` | 0.10 | MIT OR Apache-2.0 | https://github.com/RustCrypto/hashes | Cryptographic SHA-256 for migration checksums |
| `async-trait` | 0.1 | Apache-2.0 OR MIT | https://github.com/dtolnay/async-trait | Async trait methods for engine and observer |
| `reqwest` | 0.12 | Apache-2.0 OR MIT | https://github.com/seanmonstar/reqwest | Optional OpenAI-compatible local AI client |
| `gltf` | 1.4 | MIT OR Apache-2.0 | https://github.com/gltf-rs/gltf | glTF 2.0 / VRM 3D mesh and texture parser |
| `winit` | 0.30 | Apache-2.0 | https://github.com/rust-windowing/winit | Wayland / X11 cross-platform window management |
| `softbuffer` | 0.4 | MIT OR Apache-2.0 | https://github.com/rust-windowing/softbuffer | Pure-software cross-platform pixel buffer presentation |
| `url` | 2.5 | MIT OR Apache-2.0 | https://github.com/servo/rust-url | URL parsing and origin minimization |

---

## 3. Bundled and Procedural Assets

### Sample VRM 1.0 Companion Character
- **File:** Generated in `crates/wowie-avatar/tests/avatar_tests.rs` and `crates/wowie-overlay/tests/overlay_tests.rs`.
- **Format:** glTF 2.0 / VRM 1.0 (`VRMC_vrm` extension).
- **License:** Creative Commons Zero v1.0 Universal (`CC0-1.0`) / Dedicated to the public domain.
- **Attribution:** Created procedurally by the Wowie Project contributors for testing and demonstration. Contains zero proprietary character likenesses or restricted textures.

### Application Brand Icons
- **Files:** `packaging/arch/wowie.svg`, `packaging/arch/icons/wowie.png`, `extensions/browser/icons/icon-*.png`.
- **License:** `MIT OR Apache-2.0` (matching the Wowie codebase).
- **Copyright:** (c) 2024-2026 Wowie Contributors.

---

## 4. Key Transitive Dependencies

| Component | License | Author / Organization | Notes |
|---|---|---|---|
| `ring` | ISC AND OpenSSL | Brian Smith | Cryptographic primitives for rustls |
| `rustls` | Apache-2.0 OR ISC OR MIT | Rustls Contributors | Modern safe TLS implementation |
| `wayland-client` | MIT | Smithay Project | Wayland IPC client library |
| `smithay-client-toolkit`| MIT | Smithay Project | Wayland desktop shell protocols |
| `x11rb` | MIT OR Apache-2.0 | x11rb Contributors | Pure-Rust X11 protocol bindings |
| `image` | MIT OR Apache-2.0 | PistonDevelopers | Texture decoding (PNG/JPEG) |
| `miniz_oxide` | MIT OR Zlib OR Apache-2.0 | Frommi | DEFLATE compression implementation |
| `ahash` | MIT OR Apache-2.0 | Tom Kaitchuck | High-performance hash table algorithm |
| `zerocopy` | BSD-2-Clause OR Apache-2.0 OR MIT | Google LLC | Safe byte-level memory conversions |

---

## 5. Full License Texts

### MIT License

```
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Apache License, Version 2.0

```
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

### ISC License

```
Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

### Unicode License Agreement (Unicode-3.0)

```
UNICODE, INC. LICENSE AGREEMENT - DATA FILES AND SOFTWARE

See Terms of Use <https://www.unicode.org/copyright.html>
for definitions of Unicode Inc.'s Data Files and Software.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING UNICODE INC.'S DATA
FILES ("DATA FILES"), AND/OR SOFTWARE ("SOFTWARE"), YOU UNEQUIVOCALLY
ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE TERMS AND CONDITIONS OF
THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT DOWNLOAD, INSTALL, COPY,
DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

COPYRIGHT AND PERMISSION NOTICE

Copyright © 1991-2024 Unicode, Inc. All rights reserved.
Distributed under the Terms of Use in https://www.unicode.org/copyright.html.

Permission is hereby granted, free of charge, to any person obtaining
a copy of the Unicode data files and any associated documentation
(the "Data Files") or Unicode software and any associated documentation
(the "Software") to deal in the Data Files or Software
without restriction, including without limitation the rights to use,
copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Data Files or Software, and to permit persons to whom
the Data Files or Software are furnished to do so, provided that either
(a) this copyright and permission notice appear with all copies
of the Data Files or Software, or
(b) this copyright and permission notice appear in associated
Documentation.
```
