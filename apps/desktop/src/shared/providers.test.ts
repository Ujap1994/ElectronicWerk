import { describe, expect, it } from 'vitest';

import {
  isImportableDownload,
  isProviderId,
  isProviderPage,
  isProviderUrl,
  providerUrl,
  PROVIDERS,
} from './providers';

describe('online part libraries', () => {
  it('builds the pages of each provider', () => {
    expect(PROVIDERS.map((p) => p.id)).toEqual(['snapeda', 'ultralibrarian']);
    expect(providerUrl('snapeda', 'search', 'NE 555')).toBe(
      'https://www.snapeda.com/search/?q=NE%20555',
    );
    expect(providerUrl('ultralibrarian', 'search', 'TPS62130')).toBe(
      'https://app.ultralibrarian.com/search?queryText=TPS62130',
    );
    expect(providerUrl('snapeda', 'signIn')).toBe('https://www.snapeda.com/account/login/');
    expect(providerUrl('ultralibrarian', 'home')).toBe('https://app.ultralibrarian.com/');
    expect(providerUrl('snapeda', 'terms')).toBe('https://www.snapeda.com/about/terms/');
    // Tests use a local website.
    expect(providerUrl('snapeda', 'search', 'a&b', 'http://127.0.0.1:9/')).toBe(
      'http://127.0.0.1:9/snapeda/search?q=a%26b',
    );
  });

  it('only opens the providers in the browser', () => {
    expect(isProviderUrl('https://www.snapeda.com/parts/x')).toBe(true);
    expect(isProviderUrl('https://app.ultralibrarian.com/search')).toBe(true);
    expect(isProviderUrl('https://snapmagic.com/')).toBe(true);
    expect(isProviderUrl('http://www.snapeda.com/')).toBe(false);
    expect(isProviderUrl('https://snapeda.com.evil.example/')).toBe(false);
    expect(isProviderUrl('https://evilsnapeda.com/')).toBe(false);
    expect(isProviderUrl('not a url')).toBe(false);
  });

  it('knows importable downloads and valid parameters', () => {
    expect(isImportableDownload('ul_LM555.zip')).toBe(true);
    expect(isImportableDownload('part.KICAD_SYM')).toBe(true);
    expect(isImportableDownload('model.stp')).toBe(true);
    expect(isImportableDownload('datasheet.pdf')).toBe(false);
    expect(isProviderId('snapeda')).toBe(true);
    expect(isProviderId('digikey')).toBe(false);
    expect(isProviderPage('terms')).toBe(true);
    expect(isProviderPage('admin')).toBe(false);
  });
});
