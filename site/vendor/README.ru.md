[English](README.md) | [Русский](README.ru.md)

<a name="model-viewer"></a>

# model-viewer

Локально размещённые браузерные модули `@google/model-viewer` **4.3.1**:
https://registry.npmjs.org/@google/model-viewer/-/model-viewer-4.3.1.tgz

Перед копированием проверена целостность npm-архива по SHA-512:
`GP+inXhAtY31E8rILVmByA6z8CZZjdlNajddppyI1/j1eIaSQiZcMRaUqTFe7+jv4mzRzwKIOiKBud0apiv+WQ==`.

`model-viewer-module-4.3.1.min.js` — неизменённая модульная сборка upstream.
`model-viewer-LICENSE` содержит лицензию Apache-2.0 пакета. Карты исходного кода
не включены.

`model-viewer-with-effects.js` — наша небольшая точка входа: она загружает
model-viewer, затем модуль эффектов и передаёт параметры повторной загрузки
обоим импортам. После частичной ошибки уже зарегистрированные компоненты
используются повторно. Оба модуля используют общую Three.js **0.183.0** через
import map страницы:

```html
<script type="importmap">
{"imports":{"three":"./vendor/three-0.183.0/three.module.min.js"}}
</script>
```

`three.module.min.js` и соседний `three.core.min.js` — неизменённые файлы upstream;
`three-LICENSE` содержит их лицензию MIT.

Модель — обычный glTF, поэтому необязательные удалённые декодеры Draco, KTX2 и
Lottie из библиотеки не используются. Экспортируйте модели без сжатия, пока эти
декодеры не добавлены в сайт.

<a name="model-viewer-effects"></a>

## model-viewer-effects

`model-viewer-effects-1.5.0.min.js` — локальная сборка неизменённых опубликованных
JavaScript-исходников `@google/model-viewer-effects` **1.5.0** с **postprocessing
6.39.5**. Исходная готовая сборка эффектов импортирует `LuminanceFormat`, который
Three.js 0.183.0 больше не экспортирует; пересборка с совместимой версией
postprocessing позволяет не менять Three.js и не добавлять слой совместимости.

В сборке закреплены Lit **3.3.3**, lit-html **3.3.3**, lit-element **4.2.2**,
@lit/reactive-element **2.1.2**, @lit-labs/ssr-dom-shim **1.6.0** и esbuild
**0.28.2**. Архивы получены из npm-реестра и перед распаковкой проверены по SHA-512;
их URL, значения целостности, команда сборки и SHA-256 результатов — в
[`bloom-provenance.json`](bloom-provenance.json). Для работы и сборки сайта
установка через npm не нужна.

`model-viewer-effects-LICENSE` содержит Apache-2.0, `postprocessing-LICENSE` — Zlib.
`lit-LICENSE` относится к Lit, lit-element и reactive-element (BSD-3-Clause), а
`lit-html-LICENSE` сохраняет оригинальную лицензию lit-html. Комментарии об
авторских правах и лицензиях сохранены в сборках.

<a name="qrcode-generator"></a>

## qrcode-generator

`qrcode-generator-2.0.4.js` — неизменённый `dist/qrcode.js` из
[`qrcode-generator` 2.0.4](https://registry.npmjs.org/qrcode-generator/-/qrcode-generator-2.0.4.tgz).
Перед копированием проверена целостность npm-архива по SHA-512:
`mZSiP6RnbHl4xL2Ap5HfkjLnmxfKcPWpWe/c+5XxCuetEenqmNFf1FH/ftXPCtFG5/TDobjsjz6sSNL0Sr8Z9g==`.
Библиотека создаёт QR для перехода на телефон в браузере, без внешнего QR-сервиса.
Оригинальная лицензия MIT включена в `qrcode-generator-LICENSE`.
