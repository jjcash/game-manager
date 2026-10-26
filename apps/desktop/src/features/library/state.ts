// SPDX-License-Identifier: GPL-3.0-or-later
import { create } from "zustand";
interface LibraryUI {
  selection: { id: string | null; index: number } | null;
  scrollOffset: number;
  select: (id: string | null, index: number) => void;
  scroll: (offset: number) => void;
  reset: () => void;
}
export const useLibraryUI = create<LibraryUI>((set) => ({
  selection: null,
  scrollOffset: 0,
  select: (id, index) => set({ selection: { id, index } }),
  scroll: (scrollOffset) => set({ scrollOffset }),
  reset: () => set({ selection: null, scrollOffset: 0 }),
}));
