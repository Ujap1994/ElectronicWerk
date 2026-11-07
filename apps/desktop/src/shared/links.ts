/** Links to this project that the app may open in the user's browser. */
export const PROJECT_URL = 'https://github.com/Ujap1994/ElectronicWerk';

const PROJECT_URL_PATTERN = /^https:\/\/github\.com\/Ujap1994\/ElectronicWerk(?:[/?#]|$)/u;

/** `true` for https links into this project's GitHub repository. */
export function isProjectUrl(url: string): boolean {
  return PROJECT_URL_PATTERN.test(url);
}
