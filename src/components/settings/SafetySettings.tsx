export function SafetySettings() {
  return (
    <section className="settings-card">
      <h3>安全操作</h3>
      <div className="settings-row">
        <label>
          <input type="checkbox" defaultChecked /> Pull 操作需要确认
        </label>
      </div>
    </section>
  );
}
