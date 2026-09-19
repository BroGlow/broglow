[English](README.md) | [Русский](README.ru.md)

# model-viewer

Self-hosted browser modules from `@google/model-viewer` **4.3.1**:
https://registry.npmjs.org/@google/model-viewer/-/model-viewer-4.3.1.tgz

The npm archive SHA-512 integrity was verified before copying:
`GP+inXhAtY31E8rILVmByA6z8CZZjdlNajddppyI1/j1eIaSQiZcMRaUqTFe7+jv4mzRzwKIOiKBud0apiv+WQ==`.

`model-viewer-module-4.3.1.min.js` is the unmodified upstream module build.
`model-viewer-LICENSE` is the package's Apache-2.0 license. Source maps are not
shipped. The landing automatically loads 3D with Working selected.

`model-viewer-with-effects.js` is our small entrypoint: it loads model-viewer,
then the effects module, and forwards retry query parameters to both imports.
Already registered components are reused after a partial load failure.
Both modules use one shared Three.js **0.183.0** through the page's import map:

```html
<script type="importmap">
{"imports":{"three":"./vendor/three-0.183.0/three.module.min.js"}}
</script>
```

`three.module.min.js` and its sibling `three.core.min.js` are unmodified upstream
files; `three-LICENSE` contains their MIT license. Only the modular viewer and
its effects entrypoint are shipped.

The current model uses plain glTF geometry/materials, so optional remote Draco,
KTX2 and Lottie decoders in the library are not used. Keep exports uncompressed
unless those additional dependencies are explicitly made part of the site.

## model-viewer-effects

`model-viewer-effects-1.5.0.min.js` is a local build of the unmodified published
`@google/model-viewer-effects` **1.5.0** JavaScript source with **postprocessing
6.39.5**. The original prebuilt effects bundle imports `LuminanceFormat`, which
Three.js 0.183.0 no longer exports; rebuilding with the compatible postprocessing
release avoids modifying Three.js or adding a compatibility shim.

The build pins Lit **3.3.3**, lit-html **3.3.3**, lit-element **4.2.2**,
@lit/reactive-element **2.1.2**, @lit-labs/ssr-dom-shim **1.6.0**, and esbuild
**0.28.2**. Archives were downloaded directly from the official npm registry and
verified against their SHA-512 integrity before extraction. Exact source URLs,
integrity values, the build command and output SHA-256 hashes are recorded in
[`bloom-provenance.json`](bloom-provenance.json). All build work stayed in ignored
`.recovery/`; serving or building the website requires no npm install.

`model-viewer-effects-LICENSE` is Apache-2.0; `postprocessing-LICENSE` is Zlib.
`lit-LICENSE` covers Lit, lit-element and reactive-element (BSD-3-Clause), and
`lit-html-LICENSE` preserves lit-html's upstream license. Copyright/license
comments remain in the bundles. The effects use real rendered pixels, including
camera perspective and occlusion. Effects are not applied in external AR viewers
or WebXR, which use the normal model rendering.

## qrcode-generator

`qrcode-generator-2.0.4.js` is the unmodified `dist/qrcode.js` from
[`qrcode-generator` 2.0.4](https://registry.npmjs.org/qrcode-generator/-/qrcode-generator-2.0.4.tgz).
The npm archive SHA-512 integrity was verified before copying:
`mZSiP6RnbHl4xL2Ap5HfkjLnmxfKcPWpWe/c+5XxCuetEenqmNFf1FH/ftXPCtFG5/TDobjsjz6sSNL0Sr8Z9g==`.
It generates the phone handoff QR in the browser, without an external QR service.
The upstream MIT license is included in `qrcode-generator-LICENSE`.
