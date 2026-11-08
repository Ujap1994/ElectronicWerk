import { describe, expect, it } from 'vitest';

import { formatBytes, newestLibraryTag, parseManifest, releaseFileUrl } from './library';

const pack = {
  file: 'ewerk-library-kicad-10.0.8-index.zip',
  kind: 'index',
  size: 42_000_000,
  sha256: 'a'.repeat(64),
  entries: 4,
};

const manifest = {
  format: 1,
  name: 'KiCad 10.0.8',
  version: '10.0.8',
  sources: [
    { name: 'KiCad', version: '10.0.8', license: 'CC-BY-SA-4.0', url: 'https://kicad.org' },
  ],
  counts: { symbol: 22_916, footprint: 15_470, model: 7_257 },
  packs: [
    pack,
    {
      ...pack,
      file: 'ewerk-library-kicad-10.0.8-meshes-Resistor_SMD.zip',
      kind: 'meshes',
      library: 'Resistor_SMD',
    },
  ],
};

describe('parseManifest', () => {
  it('reads a valid manifest', () => {
    const release = parseManifest('library-kicad-10.0.8-r1', manifest);
    expect(release?.name).toBe('KiCad 10.0.8');
    expect(release?.counts.footprint).toBe(15_470);
    expect(release?.packs).toHaveLength(2);
    expect(release?.packs[1]?.library).toBe('Resistor_SMD');
    expect(release?.sources[0]?.license).toBe('CC-BY-SA-4.0');
  });

  it('rejects unsafe or broken data', () => {
    expect(parseManifest('v1.0.0', manifest)).toBeNull();
    expect(parseManifest('library-x', { ...manifest, format: 2 })).toBeNull();
    expect(
      parseManifest('library-x', { ...manifest, packs: [{ ...pack, file: '../evil.zip' }] }),
    ).toBeNull();
    expect(
      parseManifest('library-x', { ...manifest, packs: [{ ...pack, sha256: 'xyz' }] }),
    ).toBeNull();
    expect(
      parseManifest('library-x', { ...manifest, packs: [{ ...pack, kind: 'meshes' }] }),
    ).toBeNull();
    expect(parseManifest('library-x', null)).toBeNull();
  });
});

describe('newestLibraryTag', () => {
  it('takes the first library release and skips app releases and drafts', () => {
    const releases = [
      { tag_name: 'v0.5.0' },
      { tag_name: 'library-kicad-10.0.9-r3', draft: true },
      { tag_name: 'library-kicad-10.0.8-r2' },
      { tag_name: 'library-kicad-10.0.7-r1' },
    ];
    expect(newestLibraryTag(releases)).toBe('library-kicad-10.0.8-r2');
    expect(newestLibraryTag([{ tag_name: 'v0.5.0' }])).toBeNull();
    expect(newestLibraryTag({})).toBeNull();
  });
});

describe('helpers', () => {
  it('builds download URLs and formats sizes', () => {
    expect(releaseFileUrl('library-a', 'b c.zip')).toBe(
      'https://github.com/Ujap1994/ElectronicWerk/releases/download/library-a/b%20c.zip',
    );
    expect(formatBytes(42_000_000)).toBe('42.0 MB');
    expect(formatBytes(1_500_000_000)).toBe('1.5 GB');
    expect(formatBytes(800)).toBe('1 kB');
  });
});
