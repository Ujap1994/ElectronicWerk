export default {
  extends: ['@commitlint/config-conventional'],
  rules: {
    // Allow long trailer lines (co-author and session links).
    'footer-max-line-length': [0],
    'body-max-line-length': [0],
  },
};
