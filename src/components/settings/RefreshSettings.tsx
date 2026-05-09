export function RefreshSettings() {
  return (
    <section className="settings-card">
      <h3>刷新设置</h3>
      <div className="settings-row">
        <label>
          <input type="checkbox" defaultChecked /> 启用轻量定时刷新
        </label>
      </div>
      <div className="settings-row">
        <label>刷新间隔（分钟）</label>
        <input type="number" defaultValue={5} min={1} max={60} />
      </div>
    </section>
  );
}
