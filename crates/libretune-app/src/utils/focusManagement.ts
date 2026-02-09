/**
 * Focus Management Utilities
 * 
 * Helper functions for managing keyboard focus, focus traps, and keyboard navigation.
 * Ensures dialogs and modal elements are properly keyboard navigable.
 */

/**
 * Get all focusable elements within a container.
 * Includes buttons, links, inputs, selects, textareas, and elements with tabindex >= 0.
 */
export function getFocusableElements(container: HTMLElement): HTMLElement[] {
  const selector = [
    'button:not(:disabled)',
    'a[href]',
    'input:not(:disabled)',
    'select:not(:disabled)',
    'textarea:not(:disabled)',
    '[tabindex]:not([tabindex="-1"])',
  ].join(',');

  return Array.from(container.querySelectorAll(selector)).filter((el) => {
    const element = el as HTMLElement;
    // Check if element is visible
    return element.offsetParent !== null && 
           getComputedStyle(element).display !== 'none' &&
           getComputedStyle(element).visibility !== 'hidden';
  }) as HTMLElement[];
}

/**
 * Create a focus trap that constrains Tab/Shift+Tab to focusable elements within a container.
 * Useful for modal dialogs.
 * 
 * @param containerSelector - CSS selector or element for the dialog/modal container
 * @returns Cleanup function to remove event listener
 */
export function createFocusTrap(containerSelector: string | HTMLElement): () => void {
  const container = typeof containerSelector === 'string' 
    ? document.querySelector(containerSelector) as HTMLElement
    : containerSelector;

  if (!container) return () => {};

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key !== 'Tab') return;

    const focusableElements = getFocusableElements(container);
    if (focusableElements.length === 0) return;

    const firstElement = focusableElements[0];
    const lastElement = focusableElements[focusableElements.length - 1];
    const activeElement = document.activeElement as HTMLElement;

    // Shift+Tab on first element - focus last element
    if (e.shiftKey && activeElement === firstElement) {
      e.preventDefault();
      lastElement.focus();
    }
    // Tab on last element - focus first element
    else if (!e.shiftKey && activeElement === lastElement) {
      e.preventDefault();
      firstElement.focus();
    }
  };

  container.addEventListener('keydown', handleKeyDown);

  return () => container.removeEventListener('keydown', handleKeyDown);
}

/**
 * Set focus to the first focusable element in a container.
 * Useful when opening dialogs.
 * 
 * @param containerSelector - CSS selector or element
 * @param initialSelector - Optional selector for preferred initial focus element
 */
export function focusFirstElement(
  containerSelector: string | HTMLElement,
  initialSelector?: string
): void {
  const container = typeof containerSelector === 'string'
    ? document.querySelector(containerSelector) as HTMLElement
    : containerSelector;

  if (!container) return;

  // Try to find specific element first
  if (initialSelector) {
    const initialElement = container.querySelector(initialSelector) as HTMLElement;
    if (initialElement) {
      initialElement.focus();
      return;
    }
  }

  // Fall back to first focusable element
  const focusableElements = getFocusableElements(container);
  if (focusableElements.length > 0) {
    focusableElements[0].focus();
  }
}

/**
 * Restore focus to a previous element (useful when closing dialogs).
 * Can optionally store element reference before opening dialog.
 */
let previousFocusedElement: HTMLElement | null = null;

export function saveFocus(): void {
  previousFocusedElement = document.activeElement as HTMLElement;
}

export function restoreFocus(): void {
  if (previousFocusedElement && previousFocusedElement !== document.body) {
    previousFocusedElement.focus();
  }
  previousFocusedElement = null;
}

/**
 * Handle Escape key to close a dialog.
 * 
 * @param containerSelector - CSS selector or element for the dialog
 * @param onClose - Callback when Escape is pressed
 * @returns Cleanup function to remove event listener
 */
export function createEscapeKeyHandler(
  containerSelector: string | HTMLElement,
  onClose: () => void
): () => void {
  const container = typeof containerSelector === 'string'
    ? document.querySelector(containerSelector) as HTMLElement
    : containerSelector;

  if (!container) return () => {};

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      onClose();
    }
  };

  container.addEventListener('keydown', handleKeyDown);

  return () => container.removeEventListener('keydown', handleKeyDown);
}

/**
 * Arrow key navigation for menu items or list items.
 * Moves focus up/down through elements with arrow keys.
 * 
 * @param containerSelector - CSS selector or element for the menu/list
 * @param itemSelector - CSS selector for individual menu/list items
 * @returns Cleanup function to remove event listener
 */
export function createArrowKeyNavigation(
  containerSelector: string | HTMLElement,
  itemSelector: string
): () => void {
  const container = typeof containerSelector === 'string'
    ? document.querySelector(containerSelector) as HTMLElement
    : containerSelector;

  if (!container) return () => {};

  const handleKeyDown = (e: KeyboardEvent) => {
    if (!['ArrowUp', 'ArrowDown', 'Home', 'End'].includes(e.key)) return;

    const items = Array.from(container.querySelectorAll(itemSelector)) as HTMLElement[];
    if (items.length === 0) return;

    const activeElement = document.activeElement as HTMLElement;
    const currentIndex = items.indexOf(activeElement);

    let nextIndex = currentIndex;

    switch (e.key) {
      case 'ArrowUp':
        e.preventDefault();
        nextIndex = currentIndex > 0 ? currentIndex - 1 : items.length - 1;
        break;
      case 'ArrowDown':
        e.preventDefault();
        nextIndex = currentIndex < items.length - 1 ? currentIndex + 1 : 0;
        break;
      case 'Home':
        e.preventDefault();
        nextIndex = 0;
        break;
      case 'End':
        e.preventDefault();
        nextIndex = items.length - 1;
        break;
    }

    items[nextIndex].focus();
  };

  container.addEventListener('keydown', handleKeyDown);

  return () => container.removeEventListener('keydown', handleKeyDown);
}

/**
 * Announce messages to screen readers using ARIA live regions.
 * 
 * @param message - Text to announce
 * @param priority - 'polite' (default) or 'assertive' for urgent announcements
 */
export function announceToScreenReader(
  message: string,
  priority: 'polite' | 'assertive' = 'polite'
): void {
  let liveRegion = document.querySelector('[role="status"]');

  if (!liveRegion) {
    liveRegion = document.createElement('div');
    liveRegion.setAttribute('role', 'status');
    liveRegion.setAttribute('aria-live', priority);
    liveRegion.className = 'sr-only';
    document.body.appendChild(liveRegion);
  }

  liveRegion.textContent = message;

  // Clear after announcement
  setTimeout(() => {
    liveRegion!.textContent = '';
  }, 1000);
}
