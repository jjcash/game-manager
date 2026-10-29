// SPDX-License-Identifier: GPL-3.0-or-later
import { useState } from "react";
import { type Action, type Details } from "../../ipc/manual";

export function InstallationGroups({
  details,
  save,
  confirm,
}: {
  details: Details;
  save: (action: Action) => Promise<boolean>;
  confirm: (action: Action, message: string) => void;
}) {
  const [groupId, setGroupId] = useState("");
  const [name, setName] = useState("");
  const [installationId, setInstallationId] = useState("");
  const [assignment, setAssignment] = useState("");
  // Removed records cannot remain mutation targets. Keep unrelated draft text.
  const selectedGroup = details.groups.some((g) => g.id === groupId)
    ? groupId
    : "";
  const selectedInstallation = details.installations.some(
    (i) => i.id === installationId,
  )
    ? installationId
    : "";
  const selectedAssignment = details.groups.some((g) => g.id === assignment)
    ? assignment
    : "";
  const label = (g: Details["groups"][number]) =>
    `${g.displayName} · ${g.id.slice(0, 8)}`;
  function reset() {
    setGroupId("");
    setName("");
  }
  return (
    <details className="installation-groups">
      <summary>Organize installation groups ({details.groups.length})</summary>
      <p>
        Groups organize copies of this game across memberships. Locations are
        user-provided metadata. Grouping does not verify files, presence,
        ownership or launchability.
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save({
            kind: "saveGroup",
            groupId: selectedGroup || null,
            displayName: name,
          }).then((ok) => {
            if (ok) reset();
          });
        }}
      >
        <label>
          Group to edit
          <select
            value={selectedGroup}
            onChange={(e) => {
              setGroupId(e.target.value);
              setName(
                details.groups.find((g) => g.id === e.target.value)
                  ?.displayName ?? "",
              );
            }}
          >
            <option value="">New group</option>
            {details.groups.map((g) => (
              <option key={g.id} value={g.id}>
                {label(g)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Group name
          <input
            required
            maxLength={80}
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <button
          type="submit"
          disabled={!selectedGroup && details.groups.length >= 100}
        >
          {selectedGroup ? "Save group name" : "Create group"}
        </button>
        <button type="button" onClick={reset}>
          Cancel group edit
        </button>
        {selectedGroup && (
          <button
            type="button"
            onClick={() =>
              confirm(
                { kind: "removeGroup", groupId: selectedGroup },
                `Remove group ${details.groups.find((g) => g.id === selectedGroup)?.displayName} · ${selectedGroup.slice(0, 8)}? Its ${details.installations.filter((i) => i.groupId === selectedGroup).length} installation records become ungrouped. Installation metadata and files remain untouched.`,
              )
            }
          >
            Remove group
          </button>
        )}
      </form>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save({
            kind: "assignGroup",
            installationId: selectedInstallation,
            groupId: selectedAssignment || null,
          });
        }}
      >
        <label>
          Installation to organize
          <select
            required
            value={selectedInstallation}
            onChange={(e) => {
              setInstallationId(e.target.value);
              setAssignment(
                details.installations.find((i) => i.id === e.target.value)
                  ?.groupId ?? "",
              );
            }}
          >
            <option value="">Choose an installation record</option>
            {details.installations.map((i) => (
              <option key={i.id} value={i.id}>
                {i.rootPathRef} · {i.id.slice(0, 8)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Assigned group
          <select
            value={selectedAssignment}
            onChange={(e) => setAssignment(e.target.value)}
          >
            <option value="">Ungrouped</option>
            {details.groups.map((g) => (
              <option key={g.id} value={g.id}>
                {label(g)}
              </option>
            ))}
          </select>
        </label>
        <button type="submit" disabled={!selectedInstallation}>
          Save group assignment
        </button>
        <button
          type="button"
          onClick={() => {
            setInstallationId("");
            setAssignment("");
          }}
        >
          Cancel assignment
        </button>
      </form>
    </details>
  );
}
