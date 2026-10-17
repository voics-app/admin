import { useCallback, useEffect, useRef, useState } from "react";

export interface AsyncState<T> {
  data: T | undefined;
  error: Error | undefined;
  loading: boolean;
  reload: () => void;
}

/** Loads on mount and whenever `deps` change; optionally refreshes every `pollMs`. */
export function useAsync<T>(load: () => Promise<T>, deps: readonly unknown[], pollMs?: number): AsyncState<T> {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<Error>();
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);
  const loader = useRef(load);
  loader.current = load;

  useEffect(() => {
    let cancelled = false;
    const run = () => {
      loader
        .current()
        .then((value) => {
          if (cancelled) return;
          setData(value);
          setError(undefined);
        })
        .catch((reason: unknown) => {
          if (!cancelled) setError(reason instanceof Error ? reason : new Error(String(reason)));
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    };
    setLoading(true);
    run();
    const timer = pollMs ? setInterval(run, pollMs) : undefined;
    return () => {
      cancelled = true;
      if (timer) clearInterval(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, pollMs, tick]);

  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { data, error, loading, reload };
}
