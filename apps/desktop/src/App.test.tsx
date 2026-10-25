import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import fixture from "./ipc/app-status.fixture.json";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(cleanup);

describe("desktop shell", () => {
  it("shows the backend response and navigates clearly labeled placeholders", async () => {
    vi.mocked(invoke).mockResolvedValue(fixture);
    render(<App />);
    expect(await screen.findByText(/Rust backend connected/)).toBeTruthy();
    for (const name of ["Mods", "Settings", "Library"]) {
      fireEvent.click(screen.getByRole("button", { name }));
      expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(name);
      expect(screen.getByText("Planned · not implemented yet")).toBeTruthy();
    }
  });
  it("displays an IPC error and recovers when retried", async () => {
    vi.mocked(invoke)
      .mockRejectedValueOnce(new Error("unavailable"))
      .mockResolvedValueOnce(fixture);
    render(<App />);
    expect(
      await screen.findByText(/Could not reach the desktop backend/),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toContain("connected"),
    );
  });
});
