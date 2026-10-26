// SPDX-License-Identifier: GPL-3.0-or-later
import { defaultCriteria, type LibraryInfo } from "../../ipc/library";
import { useLibraryUI } from "./state";

export function LibraryControls({
  sources = [],
}: {
  sources?: LibraryInfo["sources"];
}) {
  const criteria = useLibraryUI((s) => s.criteria);
  const set = useLibraryUI((s) => s.setCriteria);
  return (
    <div className="library-controls" aria-label="Library query controls">
      <label className="title-search">
        Search titles
        <input
          type="search"
          aria-label="Search titles"
          placeholder="Find a game…"
          maxLength={200}
          value={criteria.search}
          onChange={(e) => set({ ...criteria, search: e.target.value })}
        />
      </label>
      <button
        className="check-button"
        onClick={() => set({ ...criteria, search: "" })}
      >
        Clear search
      </button>
      <label>
        Installation
        <select
          aria-label="Installation filter"
          value={criteria.installed}
          onChange={(e) =>
            set({
              ...criteria,
              installed: e.target.value as typeof criteria.installed,
            })
          }
        >
          <option value="all">Any installation</option>
          <option value="installed">Installed</option>
          <option value="notInstalled">Not installed</option>
        </select>
      </label>
      <label>
        Library source
        <select
          aria-label="Library source filter"
          value={criteria.sourceId ?? ""}
          onChange={(e) =>
            set({ ...criteria, sourceId: e.target.value || null })
          }
        >
          <option value="">All sources</option>
          {sources.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
      </label>
      <label>
        Year from
        <input
          type="number"
          aria-label="Release year from"
          min={1}
          max={9999}
          value={criteria.yearFrom ?? ""}
          onChange={(e) =>
            set({
              ...criteria,
              yearFrom: e.target.value ? Number(e.target.value) : null,
            })
          }
        />
      </label>
      <label>
        Year to
        <input
          type="number"
          aria-label="Release year to"
          min={1}
          max={9999}
          value={criteria.yearTo ?? ""}
          onChange={(e) =>
            set({
              ...criteria,
              yearTo: e.target.value ? Number(e.target.value) : null,
            })
          }
        />
      </label>
      <label>
        Sort
        <select
          aria-label="Sort games"
          value={`${criteria.sort}:${criteria.direction}`}
          onChange={(e) => {
            const [sort, direction] = e.target.value.split(":") as [
              typeof criteria.sort,
              typeof criteria.direction,
            ];
            set({ ...criteria, sort, direction });
          }}
        >
          <option value="title:asc">Title A–Z</option>
          <option value="title:desc">Title Z–A</option>
          <option value="releaseDate:asc">
            Release oldest · unknown first
          </option>
          <option value="releaseDate:desc">
            Release newest · unknown last
          </option>
        </select>
      </label>
      <button
        className="check-button"
        onClick={() => set({ ...defaultCriteria })}
      >
        Reset all
      </button>
    </div>
  );
}
