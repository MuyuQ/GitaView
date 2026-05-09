import type { RepoStatus } from "../types";

export function GroupFilters({ repos, selected, onSelect }: { repos: RepoStatus[]; selected: string; onSelect: (g: string) => void }) {
  const groups = ["全部分组", ...Array.from(new Set(repos.map((r) => r.group)))];
  return (
    <div className="filter-row group-filters">
      {groups.map((g) => {
        const count = g === "全部分组" ? repos.length : repos.filter((r) => r.group === g).length;
        return (
          <button
            key={g}
            className={`filter-btn ${g === selected ? "active" : ""}`}
            onClick={() => onSelect(g)}
          >
            {g} {count}
          </button>
        );
      })}
    </div>
  );
}
