<a name="clawlight-diy-website"></a>

# Сайт ClawLight DIY

[English](README.md) | [Русский](README.ru.md)

Главная страница написана на обычных HTML/CSS. Двуязычное руководство использует
[MkDocs](https://www.mkdocs.org/), [Material](https://squidfunk.github.io/mkdocs-material/)
и [static-i18n](https://ultrabug.github.io/mkdocs-static-i18n/).

<a name="edit-the-guide"></a>

## Редактирование руководства

Редактируйте Markdown-исходники; сгенерированный HTML не правьте и не коммитьте.

| Тема | Английский | Русский |
| --- | --- | --- |
| Детали, модели и проводка | `guide/en/index.md` | `guide/ru/index.md` |
| Установка, запрос агенту и Bluetooth | `guide/en/installation.md` | `guide/ru/installation.md` |
| Интеграция Codex | `guide/en/codex.md` | `guide/ru/codex.md` |
| Решение проблем и запрос агенту | `guide/en/troubleshooting.md` | `guide/ru/troubleshooting.md` |
| Справочник по оборудованию | `../docs/hardware-setup.md` | `../docs/hardware-setup.ru.md` |
| Прошивка, релизы, изображения, проектирование корпуса | `../docs/NAME.md` | `../docs/NAME.ru.md` |
| Печать и сборка | `../models/clawlight/PRINT_GUIDE.en.md` | `../models/clawlight/PRINT_GUIDE.md` |

Чтобы добавить страницу, создайте одноимённые файлы в обеих языковых папках и
обновите `nav` и `nav_translations` в `mkdocs.yaml`. Запросы агенту с кнопкой
копирования оформляйте ограждёнными блоками `text`.

<a name="keep-the-instructions-in-sync"></a>

## Синхронизация инструкций

Полные руководства из `docs/` и инструкции печати включаются в соответствующие
языковые страницы через PyMdown Snippets. Редактируйте эти исходники один раз.
Краткие руководства также включают именованные разделы, например:

```text
--8<-- "docs/hardware-setup.ru.md:lamp-wiring"
```

Сохраняйте маркеры `start:` / `end:` для `lamp-wiring`, `path-check` и `path-setup`
в обеих версиях руководства по оборудованию. Команды, GPIO, номиналы резисторов,
поля протокола и явные ID заголовков должны соответствовать друг другу в переводах.

Каждый авторский документ проекта должен иметь версии EN/RU; лицензии зависимостей
сохраняйте дословно. При изменении поведения обновляйте затронутые сводки и запросы
агенту в `guide/en/` и `guide/ru/`. Перед коммитом выполните проверки ниже и сборку
Pages. Проверка переводов выявляет отсутствующие пары, якоря и источники нужного
языка; отсутствие раздела для включения останавливает сборку.

<a name="build-and-preview"></a>

## Сборка и предварительный просмотр

Выполняйте из **корня репозитория clawlight**. В CI используется Python 3.14.

```bash
python3 -m venv site/.venv
site/.venv/bin/python -m pip install -r site/requirements-docs.txt
site/.venv/bin/mkdocs build --strict -f site/mkdocs.yaml
python3 -m http.server 8080 --bind 127.0.0.1
```

Откройте http://127.0.0.1:8080/site/ для главной страницы и
http://127.0.0.1:8080/site/docs/ для руководства. После правок Markdown повторите
сборку; результат находится в игнорируемой `site/docs/`, главная страница не очищается.

Для автоматического обновления руководства используйте
`site/.venv/bin/mkdocs serve -f site/mkdocs.yaml`. Он обслуживает только руководство;
главную страницу и локальные скачивания проверяйте через сервер из корня выше.

<a name="interactive-3d-and-ar"></a>

## Интерактивная 3D-модель и AR

**«Посмотреть в 3D»** загружает локальный `@google/model-viewer` 4.3.1 из `vendor/`
и `assets/clawlight.glb`. Статичный рендер доступен без JavaScript и при ошибке
загрузки. Управление камерой и сброс вида используют штатные возможности компонента.

AR использует `webxr`, `scene-viewer` и `quick-look` с `ar-scale="fixed"`; единицы
GLB — метры. Safari на iPhone/iPad создаёт USDZ по запросу (`ios-src` отсутствует);
другие браузеры iOS не проверены. WebXR требует HTTPS или loopback; телефон
не может обратиться к localhost другого устройства. Размещение и размер на
настоящих телефонах ещё требуют приёмки. См. [справочник AR компонента](https://modelviewer.dev/examples/augmentedreality/).

Модель показывает наш кандидат корпуса к выпуску 1.0.0 с иллюстративной
электроникой. Для печати используйте отдельный пакет корпуса. Хеши исходников и
габариты находятся в `assets/clawlight-model.json`; оба результата нужно
перегенерировать, а не править вручную. В полной рабочей копии активируйте
Python-окружение CAD и выполните:

```bash
python -m pip install -r models/clawlight/requirements.txt
python scripts/export-web-model.py
python scripts/export-web-model.py --check
```

Для экспорта нужен CAD-скилл в `~/.agents/skills/cad` либо `--cad-skill /path/to/cad`.
`--check` использует только стандартную библиотеку и проверяет хеши исходников
при наличии CAD; в сокращённых экспортах сообщает о пропущенных сравнениях.
Обычная сборка сайта использует GLB из репозитория без CAD-зависимостей.
При добавлении сжатия Draco/KTX размещайте кодеки локально. Источники и лицензии
внешних библиотек находятся в `vendor/README.md`.

<a name="languages-and-behavior"></a>

## Языки и поведение

Выбирается первый поддерживаемый язык браузера RU/EN, иначе английский.
Ручной выбор имеет приоритет и хранится в общем ключе localStorage
`clawlight-language`; удалите его для возврата к определению языка.
`guide/assets/language.js` сохраняет страницу/раздел и использует
`?lang=ru` / `?lang=en` при заблокированном хранилище. Без JavaScript ссылки
работают на языке URL, запросы можно копировать вручную.

Material предоставляет навигацию, локальный поиск и кнопки копирования;
`copy.js` добавляет доступную обратную связь об успехе или ошибке. Шрифты
системные; аналитика и внешние сервисы шрифтов не настроены.

<a name="check-changes"></a>

## Проверка изменений

```bash
site/.venv/bin/python site/check-translations.py
node site/i18n.test.cjs
node site/model-preview.test.cjs
node site/docs-language.test.cjs
node site/docs-copy.test.cjs
site/.venv/bin/python site/pages.test.py
site/.venv/bin/mkdocs build --strict -f site/mkdocs.yaml
python3 site/check-links.py
python3 site/check-links.py --base-url http://127.0.0.1:8080/site/
```

Проверка ссылок охватывает собранные страницы, якоря, языковые ссылки и скачивания.
Для HTTP-проверки оставьте сервер предварительного просмотра запущенным.

<a name="local-downloads-and-factual-limits"></a>

## Локальные скачивания и границы подтверждённых фактов

`pages.py` раскрывает `<!-- mac-downloads -->` и `<!-- model-download -->`:
локальный просмотр ссылается на новейший полный архив Mac в `dist/` с соседним
`.sha256` и на `models/clawlight-print-pack.zip`, если он есть. Без архивов
предлагаются инструкции для исходников. Публичная сборка ссылается на Releases
и ZIP исходников. Просмотр не пересобирает и не копирует архивы; архитектура,
дата и исходники пакета указаны в его `BUILD.json`.

Питание от USB Mac поддерживается; блок питания и корпус необязательны.
Прошивка предназначена только для ESP32-C3 SuperMini. `connected` означает
запись службы в лампу, но не проверку LED или хуков Codex. Доставка событий
требует реальной локальной задачи; облачные/удалённые задачи и обычные чаты
ChatGPT не управляют службой. Результаты проверок аппаратуры и службы см. в
[аппаратной приёмке](../docs/hardware-setup.ru.md#physical-and-background-service-acceptance).
Финальная проверка сборки корпуса ещё не завершена; проверки сайта и рендеры
не подтверждают физическую посадку деталей.

Источники наших изображений:

- `assets/clawlight.png`: `site/assets/clawlight.glb`, рендер закреплённой версией model-viewer с камерой главного блока.
- `assets/assembly.png`: `models/clawlight/preview/flat_floor_exploded_20260918T101601Z.png`.
- `guide/assets/favicon.svg`: наш рисунок лампы из трёх кругов, также используемый в favicon главной страницы. Значки трея берутся из `assets/icons/` в корне репозитория.

Следуйте [правилам медиа и списку фотографий](../docs/media.ru.md). Точно различайте
рендеры и фотографии; сохраняйте лицензионные уведомления темы и библиотек.

<a name="publish-to-github-pages"></a>

## Публикация в GitHub Pages

1. Следуйте [руководству по выпуску](../docs/releasing.ru.md) и отправьте нужные
   исходники, включая CAD, в ветку по умолчанию своего репозитория.
2. Выберите **Settings → Pages → Build and deployment → Source → GitHub Actions**.
3. Запустите **Actions → Website → Run workflow** для ветки по умолчанию.
4. Откройте URL задания `deploy`. При необходимости разрешите ветку по умолчанию
   и подтвердите развёртывание в окружении `github-pages`.

[Workflow Website](../.github/workflows/pages.yaml) публикует главную страницу
по адресу `https://OWNER.github.io/REPOSITORY/`, руководства — в `docs/` и `docs/ru/`.
Правки сайта/документации в `main` публикуются автоматически; для другой основной
ветки измените `push.branches`. PR проходят проверку без публикации. Свои домены
задаются в настройках Pages; относительные пути работают в корне домена и под
префиксом репозитория.

<a name="downloads-live-in-github-releases"></a>

### Скачиваемые файлы находятся в GitHub Releases

Публичные скачивания используют Releases этого репозитория с вариантом сборки
из исходников. Публикуйте полные комплекты Mac/C3 и отдельный ZIP корпуса
с контрольными суммами; архивы только с CLI не содержат прошивку и руководство.
Имена файлов, проверки и порядок публикации — в [руководстве по выпуску](../docs/releasing.ru.md).
Сохраняйте имена архивов и указывайте незавершённые физические проверки
в примечаниях к релизу. Сборка сайта не публикует релиз приложения.

<a name="check-the-exact-pages-artifact-locally"></a>

### Локальная проверка точного артефакта Pages

Замените `OWNER/REPOSITORY`; запускайте из полной рабочей копии. Доступ к GitHub не нужен:

```bash
site/.venv/bin/python site/pages.py --repository OWNER/REPOSITORY
python3 site/check-links.py --site-dir site/_pages
python3 -m http.server 8081 --bind 127.0.0.1 --directory site/_pages
```

Откройте http://127.0.0.1:8081/. Загружается только игнорируемая `site/_pages/`:
главная страница, собранная документация и ресурсы, включая GLB и его метаданные.
CAD, файлы печати, пакеты приложения и Python-окружение остаются вне артефакта.
Публичная сборка оставляет переведённые руководства на сайте, остальные ссылки
на исходники направляет в GitHub. После публикации проверьте GitHub Actions,
работающий сайт и скачивание релизов.
