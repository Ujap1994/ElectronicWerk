/**
 * The online part libraries ElectronicWerk works with. Both are used the
 * way their terms allow: the user searches and downloads on the provider's
 * website (in a provider window or in their own browser); ElectronicWerk
 * only imports the files the user downloaded. Nothing is searched or
 * downloaded automatically.
 */

export type ProviderId = 'snapeda' | 'ultralibrarian';

/** Which page of a provider to open. */
export type ProviderPage = 'home' | 'signIn' | 'search' | 'terms';

export interface Provider {
  readonly id: ProviderId;
  readonly name: string;
  /** Start page. */
  readonly home: string;
  /** Sign-in page. */
  readonly signIn: string;
  /** Search page; `{q}` is replaced by the URL-encoded query. */
  readonly search: string;
  /** Terms of use of the website and the downloads. */
  readonly terms: string;
  /** Hosts of the provider (and their subdomains). */
  readonly hosts: readonly string[];
}

export const PROVIDERS: readonly Provider[] = [
  {
    id: 'snapeda',
    name: 'SnapEDA',
    home: 'https://www.snapeda.com/',
    signIn: 'https://www.snapeda.com/account/login/',
    search: 'https://www.snapeda.com/search/?q={q}',
    terms: 'https://www.snapeda.com/about/terms/',
    hosts: ['snapeda.com', 'snapmagic.com'],
  },
  {
    id: 'ultralibrarian',
    name: 'Ultra Librarian',
    home: 'https://app.ultralibrarian.com/',
    signIn: 'https://app.ultralibrarian.com/Account/Login',
    search: 'https://app.ultralibrarian.com/search?queryText={q}',
    terms: 'https://www.ultralibrarian.com/',
    hosts: ['ultralibrarian.com'],
  },
];

export function isProviderId(value: unknown): value is ProviderId {
  return value === 'snapeda' || value === 'ultralibrarian';
}

export function isProviderPage(value: unknown): value is ProviderPage {
  return value === 'home' || value === 'signIn' || value === 'search' || value === 'terms';
}

export function providerById(id: ProviderId): Provider {
  const found = PROVIDERS.find((provider) => provider.id === id);
  if (found === undefined) {
    throw new Error(`unknown provider ${id}`);
  }
  return found;
}

/** The URL of a provider page (`base` replaces the website in tests). */
export function providerUrl(id: ProviderId, page: ProviderPage, query = '', base?: string): string {
  if (base !== undefined) {
    const path = page === 'search' ? `search?q=${encodeURIComponent(query)}` : page;
    return `${base.replace(/\/+$/u, '')}/${id}/${path}`;
  }
  const provider = providerById(id);
  if (page === 'search') {
    return provider.search.replace('{q}', encodeURIComponent(query.trim()));
  }
  return page === 'signIn' ? provider.signIn : page === 'terms' ? provider.terms : provider.home;
}

/** `true` for https URLs on a provider's own hosts. */
export function isProviderUrl(url: string): boolean {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return false;
  }
  if (parsed.protocol !== 'https:') {
    return false;
  }
  const host = parsed.hostname.toLowerCase();
  return PROVIDERS.some((provider) =>
    provider.hosts.some((own) => host === own || host.endsWith(`.${own}`)),
  );
}

/** Files a provider download can be imported from. */
export function isImportableDownload(name: string): boolean {
  return /\.(zip|kicad_sym|kicad_mod|step|stp)$/iu.test(name);
}

/** A file the user downloaded in a provider window. */
export interface ProviderDownload {
  readonly provider: ProviderId;
  /** Where it was saved. */
  readonly path: string;
  /** File name. */
  readonly name: string;
  readonly state: 'completed' | 'failed';
  /** ElectronicWerk can import it. */
  readonly importable: boolean;
}

/** What the settings show about a provider. */
export interface ProviderStatus {
  readonly id: ProviderId;
  /** The provider window keeps a session (cookies) of this provider. */
  readonly session: boolean;
}
