import type { RepoStatus } from "../types";
import { buildGroupOptions } from "../lib/statusModel";

export function GroupFilters({ repos, selected, onSelect }: { repos: RepoStatus[]; selected: string; onSelect: (g: string) => void }) {
  const groups = buildGroupOptions(repos);
  return (
    <div className="filter-row group-filters">
      {groups.map((group) => (
        <button
          key={group.name}
          className={`filter-btn ${group.name === selected ? "active" : ""}`}
          onClick={() => onSelect(group.name)}
        >
          {group.name} {group.count}
        </button>
      ))}
    </div>
  );
}
