import { fixtureRepos } from "../../fixtures";

export function RepositorySettings() {
  return (
    <>
      <header className="settings-header">
        <div>
          <h2>仓库管理</h2>
          <p>扫描目录、手动添加仓库，并为仓库分配业务分组。</p>
        </div>
        <div className="settings-actions">
          <button>扫描目录</button>
          <button className="primary">添加仓库</button>
        </div>
      </header>
      <section className="settings-card">
        <h3>已管理仓库</h3>
        {fixtureRepos.map((repo) => (
          <article className="settings-repo" key={repo.id}>
            <strong>{repo.name}</strong>
            <span>{repo.path}</span>
            <em>{repo.group}</em>
          </article>
        ))}
      </section>
    </>
  );
}
