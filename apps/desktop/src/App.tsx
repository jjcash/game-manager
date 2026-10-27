// SPDX-License-Identifier: GPL-3.0-or-later

import { useLayoutEffect, useState } from "react";
import { BackendStatus } from "./components/BackendStatus";
import { Icon } from "./components/Icon";
import "./App.css";
import { LibraryView } from "./features/library/LibraryView";

const sections = {
  Library: {
    description: "Your games, in one place.",
    title: "A home for your collection",
    detail:
      "Library browsing will arrive in a future milestone. This is the first working desktop shell.",
  },
  Mods: {
    description: "Make each game your own.",
    title: "Mod management starts with safety",
    detail:
      "Mod inventory, profiles, and deployment are planned. No game or mod files are accessed by this preview.",
  },
  Settings: {
    description: "A space that works for you.",
    title: "Your preferences, on your terms",
    detail:
      "Application preferences and storage controls are planned. This preview uses one dark theme and does not save settings.",
  },
} as const;

export type Section = keyof typeof sections;

export default function App() {
  useLayoutEffect(() => {
    performance.mark("gm:shell-commit");
  }, []);
  const [activeSection, setActiveSection] = useState<Section>("Library");
  const section = sections[activeSection];

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <aside className="sidebar">
        <div className="brand flex items-center gap-3">
          <span className="brand-mark" aria-hidden="true">
            <Icon name="Library" />
          </span>
          <div>
            <p className="font-semibold tracking-tight">Game Manager</p>
            <p className="brand-caption">YOUR SPACE TO PLAY</p>
          </div>
        </div>
        <p className="nav-label">WORKSPACE</p>
        <nav aria-label="Main navigation" className="flex flex-col gap-2">
          {(Object.keys(sections) as Section[]).map((name) => (
            <button
              key={name}
              type="button"
              className="nav-item flex items-center gap-3"
              aria-current={activeSection === name ? "page" : undefined}
              onClick={() => setActiveSection(name)}
            >
              <Icon name={name} />
              <span>{name}</span>
            </button>
          ))}
        </nav>
        <div className="sidebar-note mt-auto">
          <span className="local-dot" aria-hidden="true" />
          <span>Local first. Yours to keep.</span>
        </div>
      </aside>
      <main id="main-content" className="main-content" tabIndex={-1}>
        <header className="page-header flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="eyebrow">YOUR WORKSPACE</p>
            <h1>{activeSection}</h1>
            <p className="muted mt-2">{section.description}</p>
          </div>
          <span className="preview-badge">Development preview · M0.3</span>
        </header>
        {activeSection === "Library" ? (
          <LibraryView />
        ) : (
          <section
            className="placeholder-panel"
            aria-labelledby="placeholder-title"
          >
            <div className="panel-heading flex items-center justify-between gap-4">
              <span className="text-sm font-medium">
                {activeSection} workspace
              </span>
              <span className="placeholder-badge">Placeholder</span>
            </div>
            <div className="empty-state">
              <div className="empty-icon" aria-hidden="true">
                <Icon name={activeSection} />
              </div>
              <p className="eyebrow">A FOUNDATION FOR WHAT COMES NEXT</p>
              <h2 id="placeholder-title">{section.title}</h2>
              <p className="muted empty-description">{section.detail}</p>
              <span className="planned-label">
                Planned · not implemented yet
              </span>
            </div>
          </section>
        )}
        <BackendStatus />
        <footer className="page-footer flex flex-wrap justify-between gap-2">
          <span>Game Manager</span>
          <span>Local library · Rust + SQLite</span>
        </footer>
      </main>
    </div>
  );
}
