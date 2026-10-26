// SPDX-License-Identifier: GPL-3.0-or-later
import { create } from "zustand";
import { defaultCriteria, type LibraryCriteria } from "../../ipc/library";
interface LibraryUI {
  criteria: LibraryCriteria;
  criteriaChangedAt: number;
  setCriteria: (criteria: LibraryCriteria) => void;
  selection: { id: string | null; index: number } | null;
  scrollOffset: number;
  select: (id: string | null, index: number) => void;
  scroll: (offset: number) => void;
  reset: () => void;
}
export const useLibraryUI = create<LibraryUI>((set) => ({
  criteria: { ...defaultCriteria },
  criteriaChangedAt: 0,
  setCriteria: (criteria) =>
    set({
      criteria,
      selection: null,
      scrollOffset: 0,
      criteriaChangedAt: performance.now(),
    }),
  selection: null,
  scrollOffset: 0,
  select: (id, index) => set({ selection: { id, index } }),
  scroll: (scrollOffset) => set({ scrollOffset }),
  reset: () => set({ selection: null, scrollOffset: 0 }),
}));
