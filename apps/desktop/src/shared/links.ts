/** This project on GitHub. */
export const PROJECT_URL = 'https://github.com/Ujap1994/ElectronicWerk';

/** The documentation site (GitHub Pages). */
export const DOCS_URL = 'https://ujap1994.github.io/ElectronicWerk/';

const PROJECT_URL_PATTERNS = [
  /^https:\/\/github\.com\/Ujap1994\/ElectronicWerk(?:[/?#]|$)/u,
  /^https:\/\/ujap1994\.github\.io\/ElectronicWerk(?:[/?#]|$)/u,
];

/** `true` for https links into this project's repository or documentation. */
export function isProjectUrl(url: string): boolean {
  return PROJECT_URL_PATTERNS.some((pattern) => pattern.test(url));
}
