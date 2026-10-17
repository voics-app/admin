import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useAsync } from "./useAsync";

describe("useAsync", () => {
  it("resolves data", async () => {
    const { result } = renderHook(() => useAsync(() => Promise.resolve(7), []));
    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.data).toBe(7));
    expect(result.current.loading).toBe(false);
  });

  it("surfaces failures", async () => {
    const { result } = renderHook(() => useAsync(() => Promise.reject(new Error("boom")), []));
    await waitFor(() => expect(result.current.error?.message).toBe("boom"));
  });

  it("reloads on demand", async () => {
    const load = vi.fn().mockResolvedValue(1);
    const { result } = renderHook(() => useAsync(load, []));
    await waitFor(() => expect(result.current.data).toBe(1));
    act(() => result.current.reload());
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  });
});
