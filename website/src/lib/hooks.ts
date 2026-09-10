import { useCallback, useEffect, useRef, useState } from 'react';

/**
 * Reports when an element has been scrolled into view, and stops watching once
 * it has. Everything on this page enters once; nothing animates on the way out,
 * because content that moves when you scroll back up is harder to read, not
 * more alive.
 */
export function useInView<T extends HTMLElement>(rootMargin = '-12% 0px -12% 0px') {
  const ref = useRef<T | null>(null);
  const [inView, setInView] = useState(false);

  useEffect(() => {
    const node = ref.current;
    if (!node || inView) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setInView(true);
          observer.disconnect();
        }
      },
      { rootMargin },
    );

    observer.observe(node);
    return () => observer.disconnect();
  }, [inView, rootMargin]);

  return { ref, inView };
}

/** Whether the visitor has asked for less motion. */
export function useReducedMotion() {
  const [reduced, setReduced] = useState(
    () => window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  );

  useEffect(() => {
    const query = window.matchMedia('(prefers-reduced-motion: reduce)');
    const onChange = () => setReduced(query.matches);
    query.addEventListener('change', onChange);
    return () => query.removeEventListener('change', onChange);
  }, []);

  return reduced;
}

/** Copies text, and reports for a moment that it did. */
export function useCopy(value: string) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const copy = useCallback(() => {
    void (async () => {
      try {
        await navigator.clipboard.writeText(value);
      } catch {
        // A browser that refuses the clipboard says nothing; the text is on
        // screen and selectable either way.
        return;
      }
      setCopied(true);
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => setCopied(false), 1600);
    })();
  }, [value]);

  return { copied, copy };
}

/** Whether the page has been scrolled past a threshold. */
export function useScrolled(threshold = 8) {
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > threshold);
    onScroll();
    window.addEventListener('scroll', onScroll, { passive: true });
    return () => window.removeEventListener('scroll', onScroll);
  }, [threshold]);

  return scrolled;
}
