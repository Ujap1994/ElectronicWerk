/**
 * A check of the visible UI for cut-off controls and texts, run before each
 * documentation screenshot when `EW_UI_AUDIT=<folder>` is set (in English and
 * German, whose texts are longer). It reports:
 *
 * - `outside`: a control (or its label) reaches out of a container that
 *   hides its overflow, or out of the window;
 * - `select`: the chosen entry of a drop-down list is wider than the list;
 * - `placeholder` / `value`: the hint or the value of a short field does not
 *   fit;
 * - `text`: the text of a button, tab, label or table header is cut off.
 */
import type { Page } from '@playwright/test';

export interface AuditFinding {
  readonly kind: 'outside' | 'select' | 'placeholder' | 'value' | 'text';
  /** Test id, else tag and class. */
  readonly element: string;
  /** The text concerned. */
  readonly text: string;
  /** How many pixels are missing. */
  readonly missing: number;
}

/** Runs in the page. */
function audit(): AuditFinding[] {
  const findings: AuditFinding[] = [];
  const canvas = document.createElement('canvas').getContext('2d');
  const describe = (element: Element): string => {
    const id = element.closest('[data-testid]')?.getAttribute('data-testid');
    const own = element.getAttribute('data-testid');
    const classes = (element.getAttribute('class') ?? '').trim();
    const name = `${element.tagName.toLowerCase()}${classes === '' ? '' : `.${classes.replace(/\s+/gu, '.')}`}`;
    return own ?? (id === null || id === undefined ? name : `${id} › ${name}`);
  };
  const shown = (element: Element): boolean => {
    if (element.closest('[inert], [aria-hidden="true"]') !== null) {
      return false;
    }
    const box = element.getBoundingClientRect();
    // Texts only for screen readers are 1 px wide.
    if (box.width <= 1.5 || box.height <= 1.5) {
      return false;
    }
    const style = getComputedStyle(element);
    return style.visibility !== 'hidden' && style.opacity !== '0' && style.clipPath === 'none';
  };
  const textWidth = (text: string, element: Element): number => {
    if (canvas === null) {
      return 0;
    }
    const style = getComputedStyle(element);
    canvas.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    return canvas.measureText(text).width;
  };
  const inner = (element: HTMLElement): number => {
    const style = getComputedStyle(element);
    return element.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
  };
  const add = (finding: AuditFinding): void => {
    if (finding.missing >= 2) {
      findings.push({ ...finding, missing: Math.round(finding.missing) });
    }
  };

  // Controls that reach out of a container hiding its overflow.
  const controls = document.querySelectorAll<HTMLElement>(
    'input:not([type="checkbox"]):not([type="radio"]):not([type="hidden"]):not([type="range"]), select, textarea, button, [role="tab"]',
  );
  for (const control of controls) {
    if (!shown(control)) {
      continue;
    }
    const box = control.getBoundingClientRect();
    let missing = Math.max(0, box.right - window.innerWidth, -box.left);
    for (let parent = control.parentElement; parent !== null; parent = parent.parentElement) {
      const style = getComputedStyle(parent);
      if (style.overflowX === 'visible') {
        continue;
      }
      const frame = parent.getBoundingClientRect();
      // Scrolling containers: only the side the user cannot scroll to.
      const scrolls = style.overflowX === 'auto' || style.overflowX === 'scroll';
      const out = scrolls
        ? Math.max(0, frame.left - box.left - parent.scrollLeft)
        : Math.max(0, box.right - frame.right, frame.left - box.left);
      missing = Math.max(missing, out);
      if (scrolls && parent.scrollWidth > parent.clientWidth + 1) {
        // A container that scrolls sideways is a problem of its own.
        add({
          kind: 'outside',
          element: describe(parent),
          text: 'scrolls sideways',
          missing: parent.scrollWidth - parent.clientWidth,
        });
      }
    }
    add({
      kind: 'outside',
      element: describe(control),
      text: control.textContent.trim().slice(0, 60),
      missing,
    });
  }

  // Drop-down lists whose chosen entry does not fit.
  for (const select of document.querySelectorAll<HTMLSelectElement>('select')) {
    if (!shown(select)) {
      continue;
    }
    const text = select.selectedOptions[0]?.text ?? '';
    // The arrow takes about 18 px unless the padding makes room for it.
    const style = getComputedStyle(select);
    const arrow = parseFloat(style.paddingRight) >= 16 ? 0 : 18;
    add({
      kind: 'select',
      element: describe(select),
      text,
      missing: textWidth(text, select) - (inner(select) - arrow),
    });
  }

  // Fields whose hint or value does not fit (short values only: a long
  // text in a long field is expected to scroll).
  for (const input of document.querySelectorAll<HTMLInputElement>(
    'input:not([type="checkbox"]):not([type="radio"]):not([type="hidden"]):not([type="range"]):not([type="color"]):not([type="file"])',
  )) {
    if (!shown(input)) {
      continue;
    }
    if (input.value === '' && input.placeholder !== '') {
      add({
        kind: 'placeholder',
        element: describe(input),
        text: input.placeholder,
        missing: textWidth(input.placeholder, input) - inner(input),
      });
    } else if (input.value.length > 0 && input.value.length <= 24) {
      add({
        kind: 'value',
        element: describe(input),
        text: input.value,
        missing: textWidth(input.value, input) - inner(input),
      });
    }
  }

  // Cut-off texts of buttons, tabs, labels and headers.
  for (const element of document.querySelectorAll<HTMLElement>(
    'button, [role="tab"], label, legend, th, .field__label, .toolbar__label, h1, h2, h3, h4',
  )) {
    if (!shown(element) || element.closest('svg') !== null) {
      continue;
    }
    const style = getComputedStyle(element);
    const clips = style.overflowX !== 'visible' || style.textOverflow === 'ellipsis';
    if (!clips) {
      continue;
    }
    const text = element.textContent.trim();
    if (text === '') {
      continue;
    }
    add({
      kind: 'text',
      element: describe(element),
      text: text.slice(0, 80),
      missing: element.scrollWidth - element.clientWidth,
    });
  }
  return findings;
}

/** The findings of the page as it is now. */
export async function auditPage(page: Page): Promise<AuditFinding[]> {
  return page.evaluate(audit);
}
