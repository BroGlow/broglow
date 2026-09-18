# model-viewer

Self-hosted browser bundle from `@google/model-viewer` **4.3.1**:
https://registry.npmjs.org/@google/model-viewer/-/model-viewer-4.3.1.tgz

The npm archive SHA-512 integrity was verified before copying:
`GP+inXhAtY31E8rILVmByA6z8CZZjdlNajddppyI1/j1eIaSQiZcMRaUqTFe7+jv4mzRzwKIOiKBud0apiv+WQ==`.

The module is unmodified; its copyright/license comments remain intact.
`model-viewer-LICENSE` is the package's Apache-2.0 license. This includes the
bundled renderer; no framework or npm install is required to build this site.
Source maps are not shipped. Load the module only after the visitor requests 3D.

The current model uses plain glTF geometry/materials, so optional remote Draco,
KTX2 and Lottie decoders in the library are not used. Keep exports uncompressed
unless those additional dependencies are explicitly made part of the site.
