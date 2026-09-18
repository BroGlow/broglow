#!/usr/bin/env python3
"""Check isolated CAD staging and release guards without CAD dependencies or hardware."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile

spec = importlib.util.spec_from_file_location(
    'package_enclosure', Path(__file__).resolve().parents[1]/'scripts/package-enclosure.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


def rejected(action, message):
    try:
        action()
    except SystemExit as error:
        assert message in str(error), error
    else:
        raise AssertionError('Unsafe input accepted')


with tempfile.TemporaryDirectory(prefix='clawlight-enclosure-check-') as folder:
    package.ROOT = root = Path(folder).resolve()/'checkout'
    root.mkdir()
    for path in {Path('LICENSE'), *(Path(name) for name in package.prints.REFERENCE_FILES),
                 *(package.MODEL/name for name in package.CAD_FILES)}:
        target = root/path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text('source\n')
    model = root/package.MODEL
    (model/'PRINT_GUIDE.en.md').write_text('[Review](preview/review.png)\n[Dimensions](preview/dimensions.png)\n')
    (model/'preview').mkdir()
    (model/'preview/review.png').write_bytes(b'review')
    (model/'preview/dimensions.png').write_bytes(b'stale dimensions')
    (model/'private.py').write_text('private analysis')
    (model/'old.STL').write_bytes(b'stale geometry')
    (model/'validation.json').write_text('stale report')
    (root/'.gitignore').write_text('dist/\n')
    files = package.source_files()
    assert package.MODEL/'preview/review.png' in files
    assert all(path.name not in ('private.py', 'old.STL', 'validation.json', 'dimensions.png') for path in files)
    stage = package.prepare_source(root/'dist/enclosure', files)
    assert not (stage/package.MODEL/'old.STL').exists()
    assert not (stage/package.MODEL/'private.py').exists()
    (stage/package.MODEL/'stale.step').write_bytes(b'old generated part')
    assert package.prepare_source(root/'dist/enclosure', files) == stage
    assert not (stage/package.MODEL/'stale.step').exists()
    unknown = root/'dist/unknown/source'
    unknown.mkdir(parents=True)
    rejected(lambda: package.prepare_source(unknown.parent, files), 'unknown directory')
    blocked = root.parent/'source'
    original = package.ROOT
    package.ROOT = blocked
    rejected(lambda: package.prepare_source(root.parent, files), 'replace the source checkout')
    package.ROOT = original
    assert package.identity('1.2.3', None) == (None, None)
    rejected(lambda: package.identity('1.2.3', 'v1.2.3'), 'requires a Git checkout')

    def git(*args):
        return subprocess.check_output(['git', *args], cwd=root, text=True, stderr=subprocess.PIPE).strip()

    git('init')
    git('config', 'user.name', 'Enclosure Check')
    git('config', 'user.email', 'enclosure-check@example.invalid')
    git('add', '.')
    git('commit', '-m', 'source')
    git('tag', 'v1.2.3')
    guard = root/'scripts/check-generated-artifacts.py'
    guard.write_bytes((Path(__file__).resolve().parents[1]/'scripts/check-generated-artifacts.py').read_bytes())
    retired = [
        'retired/old.ZIP', 'elsewhere/part.StP', 'outside/mesh.sTl', 'view.STEP',
        'models/clawlight/cad-inspection.json', 'models/clawlight/coupon-inspection.json',
        'models/clawlight/USB-COUPON-INSPECTION.JSON',
        'models/clawlight/preview/snapshot-job.json',
        'models/clawlight/preview/base-fit-snapshot-job.json',
        'models/clawlight/preview/cover-chamfers-snapshot-job.json',
        'models/clawlight/preview/flat-floor-snapshot-job.json',
        'models/clawlight/preview/TIGHT-USB-SNAPSHOT-JOB.JSON',
    ]
    preserved = [
        'models/clawlight/parts.json', 'models/clawlight/validation.json',
        'models/clawlight/slicer-validation.json', 'models/clawlight/slicer-result.json',
        'site/assets/clawlight-model.json', 'site/vendor/bloom-provenance.json',
        'references/clawlight/ar-component-bounds.json',
        'references/clawlight/ar-model-metadata.json', 'references/clawlight/sources.json',
        'config.json', 'tests/fixtures/cad-inspection.json',
    ]
    for name in preserved:
        path = root/name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('{}\n')
    git('add', *preserved)
    for name in retired:
        path = root/name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'generated')
    git('add', *retired)
    rejected_artifacts = subprocess.run([sys.executable, str(guard)], text=True, capture_output=True)
    assert rejected_artifacts.returncode != 0
    assert all(name in rejected_artifacts.stderr for name in retired)
    git('rm', '--cached', *retired, 'models/clawlight/old.STL')
    accepted = subprocess.run([sys.executable, str(guard)], text=True, capture_output=True)
    assert accepted.returncode == 0, accepted.stderr
    assert 'models/clawlight/preview/review.png' in git('ls-files')
    for name in retired + ['models/clawlight/old.STL']:
        (root/name).unlink()
    git('add', '.')
    git('commit', '-m', 'check source tracking policy')
    git('tag', '-f', 'v1.2.3')
    assert package.identity('1.2.3', 'v1.2.3') == (git('rev-parse', 'HEAD'), False)
    (model/'clawlight.py').write_text('changed source\n')
    rejected(lambda: package.identity('1.2.3', 'v1.2.3'), 'clean source')
    (model/'clawlight.py').unlink()
    (model/'clawlight.py').symlink_to(model/'private.py')
    rejected(package.source_files, 'unsafe source input')
print('PASS: isolated staging, release guards, mixed-case artifacts and scoped CAD JSON tracking')
